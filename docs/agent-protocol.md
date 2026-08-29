# Agent ↔ agentd protocol

Implementation notes for the `agent/` workspace (issues #1–#15 on the
[Agent project board](https://github.com/users/jfuerwentsches/projects/2)).
The concept source of truth stays the private vault note "Omarchy Kids -
Implementierung Agent" — this doc covers the concrete decisions made while
implementing it.

## Components

- **`omarchy-kids-common`** — shared protocol types, config schema, desktop-
  entry parsing. Used by all four binaries below.
- **`omarchy-kids-agent`** — thin CLI, reached over SSH via a
  `command=`-restricted key. Forwards to `agentd` over the local socket.
- **`omarchy-kids-agentd`** — session daemon. Owns the config, the SQLite
  usage log, time-budget enforcement, and launcher-tile rendering.
- **`omarchy-kids-run`** — the app wrapper (`omarchy-kids-run <desktop-id>`).
  Every unlocked app launches through this instead of directly.
- **`omarchy-kids-override-helper`** — the actual `pkexec` target for the PIN
  override path; never run directly.
- **`omarchy-kids-pairing`** — belongs to the Pairing track (setup-wizard
  issues #21-#24), not the original 4-binary Agent project split above, but
  lives in this workspace since it's Rust and shares little with the
  wizard's shell scripts. See "Pairing protocol" below.

## Account topology (resolved ambiguity)

The vault note's "Komponentenaufteilung" section says `agentd` "läuft ...
im separaten Admin-/Eltern-Account", which would conflict with the later
"Architektur-Begründung" section's requirement that session-affecting
actions (theme/tier switches, Quickshell IPC) must run *inside the live
session* to have any visible effect. Resolved as follows:

- **`agentd`, `agent`, and `omarchy-kids-run` all run as the child's own
  local account** (systemd `--user` service under that account) — that's
  the account with the live Wayland session and the running Quickshell
  instance, and it's also where the Unix socket under `$XDG_RUNTIME_DIR`
  naturally lives for both the SSH-invoked `agent` and the wrapper.
- **The separate parent/admin account exists only for local authentication**
  — polkit's `auth_admin` check on the override path (issue #9) prompts for
  *that* account's credentials, which is what makes "PIN entry" meaningful:
  the child's own account must not be a polkit admin (not in `wheel`),
  enforced by the setup wizard, not this package.

## Wire format

Newline-delimited JSON over a Unix domain socket at
`$XDG_RUNTIME_DIR/omarchy-kids/agentd.sock` (mode 0600, created by agentd).
One request per connection: connect, write one `Request` line, read one
`Response` line, close. See `common/src/protocol.rs` for the full enum —
`Status`, `SetTier`, `Unlock`, `Override`, `OverrideFailed`, `Report`,
`WrapperStart`/`WrapperStop`/`WrapperPoll`, and a reserved `PushContent`
placeholder (issue #13, not implemented).

`WrapperPoll` exists so agentd can end a session early (time window hit,
budget exhausted) without a reverse connection into the wrapper: the wrapper
polls every 15s while the app runs and self-terminates it (SIGTERM, then
SIGKILL after a 5s grace) if agentd says it's no longer allowed.

## Design decisions made while implementing

- **Fail-open (issue #6).** If `omarchy-kids-run` can't reach agentd at all
  (connect/timeout), it launches/keeps the app running anyway, logging a
  warning. The kiosk's real access-control boundary is the VT/getty lockdown
  (see root `CLAUDE.md`), not this wrapper — an agentd outage should degrade
  time-budget enforcement, not strand the child on a computer that silently
  does nothing. No replay/spool of missed events is implemented; a short
  outage just under-counts that session's usage.
- **`.desktop` wrapper generation on tier switch (issue #15) — turned out to
  be unnecessary.** The mini tier's only launch surface is the kiosk
  launcher grid, which now calls `omarchy-kids-run <desktop-id>` directly
  (`Launcher.qml`) instead of `gtk-launch`. There's no second launch surface
  (no stock app menu in this tier) that would need its own rewritten
  `.desktop` entries. Revisit if a later tier exposes a standard DE app
  grid alongside/instead of this launcher.
- **`launcher-apps.json` rendering moved into agentd.** `omarchy-kids-set-
  tier` still owns theme/Hyprland/VT lockdown, but now hands the tier's
  `apps.json` to agentd via `agent set-tier <tier> --apps-file <path>`
  instead of copying it directly. agentd is then the sole writer of
  `launcher-apps.json` (base tiles ++ active temporary unlocks), so a
  `unlock`/`override` call takes effect immediately without racing the
  script's own copy step. Falls back to the old direct-copy behavior if
  agentd isn't reachable (e.g. a fresh box before the daemon is enabled),
  so tier switching doesn't hard-depend on agentd being up.
- **Pre-warning transport (issue #8).** Confirmed on the dev VM: Omarchy's
  own `omarchy-shell <target> <method> [args...]` wrapper (which resolves to
  `qs ipc -n -p "$OMARCHY_PATH/shell" call ...` plus `WAYLAND_DISPLAY`
  recovery for non-session callers) reaches a running Quickshell instance's
  `IpcHandler` from agentd. Reused instead of hand-rolling a `qs ipc call` —
  see `agentd/src/prewarning.rs`. `Launcher.qml` registers target
  `omarchyKidsLauncher` with a `preWarn(app, secondsLeft)` function; agentd's
  background ticker calls it once an active session is within its tier's
  lead time (`pre_warning.lead_minutes`) of a cutoff. Mini tier shows a
  banner and attempts an acoustic cue (`canberra-gtk-play`), per the
  non-reader requirement in the parental-controls design note.
- **Security-event severity threshold (issue #10).** 3 failed override
  attempts for the same app within 5 minutes escalates an `override_failed`
  event from routine to severe. Severe events get a local `notify-send`
  right now; **cross-host delivery to the parent's computer is blocked on
  the Control Center existing** (`control/` is still a stub — see root
  `CLAUDE.md` "Status"). Routine events are only visible via `agent report`.
- **Push-content (issue #13).** `Request::PushContent { content_type }` is
  reserved in the wire enum and rejected with a clear "not implemented yet"
  error — kept so the wire format doesn't need a second redesign once photo/
  playlist transfer (see the Roadmap/Spotify vault notes) actually lands.

## UFW SSH rule ownership (issue #18)

The agent package's `post_install` (above) already runs `ufw allow ssh`
when UFW is active. The setup wizard's bootstrap script does **not**
duplicate this — one owner avoids two places doing the same thing for
unclear reasons, and the package hook already covers both fresh installs
and upgrades. The bootstrap script only logs a warning if it finds UFW
active without the rule, so a misordered install (wizard run before the
agent package, or UFW re-enabled afterwards) fails loudly instead of
leaving the child host silently unreachable for pairing. See
`setup-wizard/bootstrap/omarchy-kids-bootstrap`.

## Pairing protocol (issues #21-#24)

`omarchy-kids-pairing` implements the hub note's Pairing-Mechanismus
end-to-end: `serve` (child side, run as the child account) opens a
time-boxed window — mDNS broadcast (`_omarchy-kids-pairing._tcp.local.`,
discovery only, no secret in the TXT records) plus a QR-code fallback —
accepts exactly one connection, and installs whatever public key it
receives as the usual `command=`-restricted `authorized_keys` entry.
`pair` is the client half of the identical protocol — originally a
reference/test client standing in for the real Control Center, now what
Control Center's own `PairingDialog` actually drives as a subprocess (see
"Control Center's pairing implementation" below); this is the concrete
answer to issue #24's "Control-Center-side pairing contract".

**Key decision (changes issue #17):** the SSH keypair is generated by the
*Control Center*, not the child — only the public half ever crosses the
network, encrypted or not. `setup-wizard/bootstrap/lib/ssh-key.sh` no
longer generates or stashes a private key; it just prepares `~/.ssh`.

**Security model:** SPAKE2 (`spake2` crate — RustCrypto, no independent
audit, accepted for this threat model) authenticates the exchange using
the pairing code as the shared password. This is deliberately not "derive
a key from the PIN and encrypt" — SPAKE2 gives an on-path/LAN attacker
exactly one online guess per connection, with no offline attack on a
captured transcript, so an 8-character human-typeable code is enough.
`serve` accepts one connection per process invocation, which is itself the
rate limit: a fresh guess needs a fresh process, which needs the wizard's
explicit retry action (see setup-wizard issue #25), not something a remote
attacker can trigger unassisted. Verified end-to-end in the dev VM
(2026-08-29), including SSH login through the freshly paired key.

**UFW rule around the pairing window (resolved, setup-wizard issues
#22/#23):** the pairing port (7420 by convention) has no standing UFW rule
— correctly so, since it must be open only for the deliberate pairing
window, not permanently (same "SSH is the only open door" principle the
getty-lockdown fix in setup-wizard issue #16 already leans on). The
first-boot wizard (`setup-wizard/first-boot/omarchy-kids-setup-wizard`,
`run_pairing`) now opens/closes that rule directly around its `serve`
call — no sudoers workaround needed like `grant_getty_lockdown_sudo`,
since the wizard already runs as root at that point (unlike the getty-mask
case, where the *child* account has to issue the privileged command
later). Verified in the dev VM (2026-08-29): rule appears/disappears
correctly, and a real `serve`/`pair` round trip over the wizard's own
`sudo -u <child_user>` invocation pattern installed the key correctly.

**Control Center's pairing implementation (2026-08-29):** rather than
reimplementing SPAKE2 in C++, `control/gui/`'s `PairingDialog` shells out
to `omarchy-kids-pairing pair` — the same "call a trusted binary as a
subprocess" pattern already planned for `ssh` (see the vault note
"Omarchy Kids - Implementierung Control Center"). That required reworking
`pair` itself, since it used to auto-confirm the fingerprint as a stand-in
for a real parent decision:

- `pair` now prints the fingerprint, then blocks reading a `y`/`n` line
  from stdin instead of hardcoding `confirmed: true` — the GUI shows a
  confirmation dialog on the fingerprint line and writes the answer back
  to the subprocess's stdin. A new `--yes` flag restores the old
  auto-confirm behavior explicitly, for scripting/testing.
- `serve` now also prints the resolved host/IP (previously only encoded in
  the QR image, never as text) — without it the manual-entry fallback path
  was unusable, since the parent had no way to know what to type.
- On success, `pair` prints a final `PAIR_RESULT: {...}` JSON line
  (hostname, ssh_port, fingerprint, key_path) — parsed by the GUI via
  `QJsonDocument` instead of scraping the human-readable text.

`control/core/`'s `HostRegistry` persists paired children as TOML
(`tomlplusplus`) at `~/.config/omarchy-kids-control/hosts.toml`, matching
the vault note's own data-model sketch exactly.

Verified with a full round trip against the dev VM (GUI on the parent's
own machine, `serve` on the VM as `fine`): fingerprint shown, confirmed,
key installed, and a real SSH login through it correctly restricted to
`omarchy-kids-agent`. Two real bugs found and fixed along the way: a
`QProcess::start()` failure (binary missing from PATH) went unhandled and
silently hung the dialog forever — `errorOccurred` wasn't connected;
retrying after a failed attempt for the same child name collided with the
stale key file the first attempt's `ssh-keygen` step had already written,
and the first attempt's still-running subprocess was never killed, so its
eventual (UFW-was-blocking-it) reply landed on whatever session the retry
had since started — both now handled (stale key file removed before each
attempt, previous subprocess killed before starting a new one).

**Retry/skip behavior (setup-wizard issue #25, mostly resolved by how
`serve` already works, one real gap remains):** mDNS and the QR code are
both live for the whole pairing window — not a sequential "mDNS first, QR
as fallback after a timeout" — so there's no separate fallback state to
design. A skipped (Ctrl+C), timed-out, or failed attempt is logged as a
warning by the wizard and setup completes anyway; the child machine is
fully usable standalone without ever pairing.

**Still open:** how a parent re-triggers pairing later on a *production*
machine, without shell access. `omarchy-kids-pairing serve` has to run as
the child account, but the child account has no reachable shell by design
(that's the whole point of the kiosk lockdown) — the paired SSH channel
itself can't be used to invoke it either, since its `command=` restriction
only ever runs `omarchy-kids-agent`, and a machine that skipped pairing
has no paired channel yet anyway. Root SSH exists only in the dev VM (see
root CLAUDE.md, "dev-only convenience... unrelated to the production
agent's `command=`-restricted key design") — not a real answer.
`omarchy-kids-override-helper` shows the shape of a real fix (a `pkexec`
target the child's live session can invoke, gated on the admin account's
credentials, no full login needed) but today only covers the time-budget
PIN override, not pairing. Likely needs an equivalent small local trigger
once Control Center (or a comparable local UI) exists to drive it —
tracked in setup-wizard issue #25.

## Not yet done

- No replay of fail-open events once agentd comes back (see above).
- Concrete per-tier pre-warning lead-time values beyond mini's are still
  undecided (vault note "Noch offene Punkte") — only `mini = 1` minute is
  set as a default.
- The account-topology requirement above (child not in `wheel`, a separate
  admin account exists) is now enforced by the setup wizard's Phase 1
  bootstrap script (`setup-wizard/bootstrap/`) — see that folder's README.
  Not yet wired into an actual first-boot hook (setup-wizard issue #27).
