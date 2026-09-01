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
- **`omarchy-kids-repair-helper`** — the `pkexec` target for re-triggering
  pairing on a production machine (issue #25); never run directly. Unlike
  `override-helper`, runs as root (needs it to toggle the pairing UFW rule),
  then drops to the child account itself via `sudo -u` before invoking
  `omarchy-kids-pairing serve`. See "Pairing protocol" below.
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
- **Security-event severity threshold (issue #10), now closed.** 3 failed
  override attempts for the same app within 5 minutes escalates an
  `override_failed` event from routine to severe. Severe events get a local
  `notify-send` on the child's own session; cross-host delivery to the
  parent's computer is now handled by Control Center's dashboard (see
  "Control Center's dashboard" below) — polls the selected host's `agent
  report --week --json` on a timer and fires its own `notify-send` for any
  severe event not already surfaced this session. Routine events are shown
  passively in that same panel, matching the tiered-delivery design.
- **Push-content (issue #13).** `Request::PushContent { content_type }` is
  reserved in the wire enum and rejected with a clear "not implemented yet"
  error — kept so the wire format doesn't need a second redesign once photo/
  playlist transfer (see the Roadmap/Spotify vault notes) actually lands.
- **Found while building Control Center's dashboard, not yet exercised by
  any prior verification: remote commands never actually reached agentd.**
  The `command=` restriction in the pairing-installed `authorized_keys`
  entry points straight at the `omarchy-kids-agent` binary
  (`command="/usr/bin/omarchy-kids-agent"`, see `install_pubkey` in
  `pairing/src/main.rs`) — sshd execs that with **zero** argv regardless of
  what the client actually asked for (`ssh host status --json`), stashing
  the real request in `$SSH_ORIGINAL_COMMAND` instead. Nothing read that
  variable back out, so every remote invocation silently collapsed to a
  bare `omarchy-kids-agent` (clap's "no subcommand given" usage error, exit
  2) — prior verification only ever checked that the SSH *login* itself
  worked, never a real remote command. Fixed in `agent/src/main.rs`'s new
  `effective_args()`: when the process's own argv is empty (just argv[0]),
  it re-parses from `$SSH_ORIGINAL_COMMAND` (via the `shlex` crate) instead.
  Guarded to only kick in when argv is otherwise empty, so a local
  invocation (`override`, `repair-pairing`) is never affected even if that
  variable happens to be set for unrelated reasons.

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
accepts exactly one connection, and validates a received public key before
installing it as the usual `command=`-restricted `authorized_keys` entry.
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

**Commit protocol (security fix, issue #34):** wire protocol v2 treats the
fingerprint exchange as preparation, not authorization. The child validates
the public key and calculates its fingerprint without touching
`authorized_keys`, sends `Confirm`, and waits for `Ack { confirmed: true }`.
Only then does it atomically replace `authorized_keys` with the restricted
entry and send an authenticated `Committed { success: true }`. Declines,
disconnects, malformed acknowledgements, and failed writes therefore add no
authorized key. The client does not print `PAIR_RESULT` or otherwise report
success until that final commit result arrives. Protocol v1 is rejected:
its install-before-confirmation ordering cannot be retained safely.

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
own machine, `serve` on the VM as `devchild`): fingerprint shown, confirmed,
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

**Production re-pairing trigger (resolved, closes issue #25):**
`omarchy-kids-agent repair-pairing`, run on the child computer itself (not
over SSH — the paired channel's `command=` restriction only ever runs this
same binary's regular subcommands, and a machine that skipped pairing has
no paired channel yet anyway). Same "PIN entry at the child's computer"
shape as `override`/`omarchy-kids-override-helper`: `pkexec` gates it on
the `net.omarchykids.agent.repair-pairing` polkit action (`auth_admin` —
the separate parent/admin account, no full login needed), then runs
`omarchy-kids-repair-helper`. One deliberate difference from
`override-helper`: this helper runs as **root** (no `--user` on the
`pkexec` call), not the child account, because it also needs to toggle the
pairing UFW rule — root opens the rule, drops to the child account via
`sudo -u` to run `omarchy-kids-pairing serve` (mirroring the wizard's own
`run_pairing`/`sudo -u "$child_user"` pattern exactly), then always closes
the rule on the way out, success or not. A skipped/timed-out/failed
re-pairing attempt is reported to the parent (pkexec's exit code is
propagated) but not logged as a security event — unlike a failed PIN
override, it isn't gated on secret knowledge an attacker could brute-force
from elsewhere, so issue #10's severity escalation doesn't apply here.
Not yet wired into any UI trigger (a Quickshell-plugin button, say) — like
`override`'s own CLI, that's left for whichever kiosk-side UI ends up
calling it; `quickshell-plugin/` is still a stub (see root CLAUDE.md
"Status").

## Control Center's dashboard (issue #10's cross-host half)

`control/gui/`'s `MainWindow` grew from a bare host list into a first real
dashboard: selecting a paired child polls it over SSH (`control/core/`'s new
`AgentClient`, running `omarchy-kids-agent status --json` and `report --week
--json`) and shows tier/unlocked-apps/budget status plus a security-events
list, newest first. Severe events fire a local `notify-send` on the
*parent's* computer — the missing half of issue #10's tiered delivery
(agentd's own `notify-send` only ever reached the child's own session).

A few decisions worth recording:

- **`AgentClient` lives in `control/core` (Qt-free), not `control/gui`** —
  same reasoning as `HostRegistry`: `control/tui/` is meant to share this
  logic once it's more than a placeholder, so nothing SSH-specific belongs
  in the Qt-only layer.
- **execvp, not a shell.** `AgentClient::run` forks and `execvp`s `ssh`
  directly with an argv array instead of building a shell command string —
  `HostEntry::hostname`/`keyPath` can originate from LAN-discovered pairing
  data or manual entry, so this must never let their contents be
  interpreted as shell syntax.
- **Polling is synchronous but off the GUI thread.** `AgentClient::run`
  blocks for up to its timeout (SSH's own `ConnectTimeout=5` plus a
  wall-clock watchdog as a fail-safe); `MainWindow` runs it on
  `QThreadPool` and hops back via `QMetaObject::invokeMethod`'s
  context-object overload, which safely drops the call if the window was
  closed while a poll was in flight.
- **Notification bootstrap, not full history replay.** The first poll of a
  session for a given host only records the newest severe event's
  timestamp — it doesn't fire a notification for it. Otherwise every
  pre-existing severe event in a child's history would notify the instant
  Control Center opens, which is noise, not a real-time alert.
- **Found while wiring this up, not a Control Center bug: remote agent
  commands never actually reached agentd before this session.** See the
  new "Design decisions" bullet above (`effective_args()`/
  `$SSH_ORIGINAL_COMMAND`) — without that fix, `AgentClient::run`'s
  `status`/`report` calls would have silently gotten clap's "no subcommand
  given" usage error back instead of real data.

**Not yet done:** only the currently *selected* host is polled — there is
still no headless polling mode (see root CLAUDE.md "Status"), so a severe
event on a paired-but-unselected child won't notify until that child is
selected again. Host-key verification is plain TOFU (`StrictHostKeyChecking
=accept-new`), not pinned to the SPAKE2-confirmed fingerprint pairing
already established. App-unlock and tier-switch controls, and any actual
usage-stat charts, are still not built — this dashboard is read-only.

## End-to-end verification (issue #29, closed 2026-08-29)

The real thing, not a stand-in: a fresh Omarchy VM, deferred-provisioning
install (Ctrl+C at the keyboard-layout screen), `omarchy-kids-agent`/
`agentd`/`pairing`/the setup-wizard scripts/`omarchy-kids-setup-wizard.service`
injected onto its disk offline (via `qemu-nbd`, no packaging exists for
`setup-wizard`/`tiers` yet — see their READMEs) *before* its first real
boot, then booted for real and driven through the actual first-boot chain
via `virsh screenshot`/`send-key` (no manual `serve` invocation, no
synthetic test harness):

`omarchy-provision-owner.service` (Omarchy's own deferred account setup) →
our chained `omarchy-kids-setup-wizard.service` (gum form: name/tier/
language, on the same tty1) → `omarchy-kids-bootstrap` (parent/admin
account, wheel removal, branding, `omarchy-kids-set-tier mini`) →
`run_pairing`'s UFW-wrapped `omarchy-kids-pairing serve` window (mDNS + QR
+ code, genuinely printed to tty1) → paired from the *host* against that
real window with `omarchy-kids-pairing pair` → a real Control Center
(`control/gui/omarchy-kids-control`, launched for real on the parent's own
Wayland session, not headless) polling the newly-paired child over SSH and
showing it online with live tier/budget/security-event data.

**Two real, previously-unverified gaps found and fixed by actually running
this, not by re-reading the code:**

1. **Binary path mismatch.** The pairing-installed `command=` restriction
   hardcodes `/usr/bin/omarchy-kids-agent` (matching the real, still-unused
   `PKGBUILD`) — the dev injection had put the binaries at `/usr/local/bin`
   for expedience, so the very first `ssh ... status` attempt failed with
   "No such file or directory" despite pairing itself having succeeded.
   Fixed by placing them at `/usr/bin` to match what packaging already
   assumes.
2. **The pairing protocol never transmitted the child's username — a real
   protocol gap, not a test artifact.** `SecurePayload::Confirm` (see
   "Pairing protocol" above) carried `hostname`/`ssh_port`/`fingerprint`
   but not `username`; `HostEntry` had nowhere to store one either. Since
   the child account's name is chosen freely during Omarchy's own account
   setup and was never otherwise communicated, Control Center had no way
   to know which account's `authorized_keys` the freshly paired key lived
   in — `AgentClient::run` was building bare `ssh host ...` instead of
   `ssh user@host ...`, which authenticates as whatever *local* account
   Control Center itself runs under instead (worked by pure coincidence
   never — my own manual first test only "worked" because I already knew
   the username from having driven the wizard myself moments earlier).
   Fixed end-to-end: `serve` now reports its own `$USER`/`LOGNAME` in
   `Confirm`; `pair` surfaces it in `PAIR_RESULT` and its printed `ssh -i
   ... user@host` hint; `HostEntry`/`hosts.toml` gained a `username`
   field; `PairingDialog` stores it; `AgentClient` connects to `user@host`.
   Verified with a second real `serve`/`pair` round trip (host-to-host,
   not through the VM) confirming `PAIR_RESULT` now includes the correct
   username, then re-verified the VM dashboard poll actually working with
   a corrected `hosts.toml` entry.

Confirms, for the first time against a real deferred-provisioning boot
rather than a manually-driven one: Omarchy 4 has no first-boot hook API of
its own (issue #26's research), so systemd unit ordering really is
sufficient to chain onto it; the mini tier's SDDM-autologin-permanence fix
takes effect correctly on a genuinely fresh unencrypted install; and the
full pairing→Control-Center chain works against a real child, not a
stand-in `pair` invocation on the same box.

## Not yet done

- No replay of fail-open events once agentd comes back (see above).
- Concrete per-tier pre-warning lead-time values beyond mini's are still
  undecided (vault note "Noch offene Punkte") — only `mini = 1` minute is
  set as a default.
- The account-topology requirement above (child not in `wheel`, a separate
  admin account exists) is now enforced by the setup wizard's Phase 1
  bootstrap script (`setup-wizard/bootstrap/`) — see that folder's README.
  Not yet wired into an actual first-boot hook (setup-wizard issue #27).
