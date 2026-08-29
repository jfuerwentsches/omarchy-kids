# omarchy-kids — working notes for Claude Code

A configuration layer on top of [Omarchy](https://omarchy.org) that grows
with a child — age-tiered desktop profiles plus tooling for parental
controls and screen time. Not a fork: Omarchy installs normally, this
project layers config, a background agent, and a control center on top.

## Where the actual design lives

The maintainer keeps the concept/design source of truth in a private notes
system outside this repo — not published here, and not needed to build or
run anything in it. If you're the maintainer (or working from a session
that has access to it), it's linked from a local, gitignored
`.claude/CLAUDE.local.md` — start there for design rationale before making
architectural decisions. Everyone else: this file, the folder-level
READMEs, and commit history are the available context.

For orientation, the private notes are organized around these topics (contributors without access can safely ignore this list — it's here so issues/PRs can reference the right area by name):

- `Omarchy Kids (mitwachsendes OS)` — hub note, links everything
- `Omarchy Kids - Architekturuebersicht` — full component map + security principles
- `Omarchy Kids - Altersstufe 5-7` / `- 8-10` — per-tier concept (apps, UI principle)
- `Omarchy Kids - Implementierung Agent` — agent/agentd design + why the split matters
- `Omarchy Kids - Implementierung Control Center` — control center design
- `Omarchy Kids - Implementierung Launcher` — kiosk launcher plugin, the Quickshell gotchas
- `Omarchy Kids - Themes` — per-tier theming, franchise-licensing stance
- `Omarchy Kids - Sprache und Locales` — German-from-day-one i18n plan
- `Omarchy Kids - Entwicklungsumgebung` — dev environment design rationale
- `Omarchy Kids - Open-Source-Struktur und Paketierung` — repo/license/packaging decisions

## Current focus

**Only tier "mini" (age 5-7) is being built right now** — explicitly
deprioritized the other tiers (midi/8-10, maxi/11-13, teen/14-16) until mini
is solid. Don't start scaffolding other tiers unless asked.

## Status (as of the last session)

- Monorepo scaffold in place: `agent/` (Rust workspace, builds), `control/`
  (CMake/Qt6, builds), `tiers/`, `quickshell-plugin/`, `setup-wizard/`,
  `docs/` — see each folder's README for stack/status.
- `tiers/mini/` is a working end-to-end kiosk, verified in the dev VM:
  - `themes/` — a tier can ship multiple themes now (own artwork, no
    licensed franchises): `sternenreise` (default, space) and
    `meerjungfrauen` (underwater/mermaid). `omarchy-kids-set-tier <tier>
    [theme]` installs all of a tier's themes plus any dropped into
    `~/.config/omarchy-kids/themes/<tier>/` (same folder shape, no code
    change needed), and activates the given theme or
    `themes/default-theme.txt`.
  - `launcher/omarchy-kids.launcher/` — fullscreen Quickshell overlay plugin,
    icon-only grid, launches via `gtk-launch`. App line-up is being
    reworked (see the Altersstufe-5-7 vault note): GCompris is being forked
    as `omarkid-gcompris` for theming, KTuberling stays as-is, Tux
    Paint/Klettres/Blinken were dropped entirely — `apps.json` in this repo
    still reflects the old line-up until that lands.
  - `hypr/hyprland.lua` — full Hyprland config replacement: every default
    binding off, only `SUPER+SPACE` survives (→ the kiosk launcher)
  - `omarchy-kids-set-tier` — applies all of the above, plus masks
    `getty@tty2-6` (VT-switch lockdown) for this tier
- `agent/` (issues #1-#15) is implemented, not a stub: `agent`/`agentd`/
  `omarchy-kids-run`/`omarchy-kids-override-helper` — protocol, time
  budgets, PIN override, packaging. See `docs/agent-protocol.md`.
- `setup-wizard/` (issues #16-#27 done so far, project board:
  [Pairing & Setup Wizard](https://github.com/users/jfuerwentsches/projects/3))
  also has real logic now, not just a stub:
  - `setup-wizard/bootstrap/` — Phase 1 scripted bootstrap (account/wheel
    topology, branding, initial tier switch), verified end-to-end in the
    dev VM. See its README.
  - `agent/pairing/` (`omarchy-kids-pairing` binary, lives in the agent
    workspace though it's the setup-wizard's Pairing track) — child-to-
    Control-Center pairing exchange (mDNS + QR discovery, SPAKE2-
    authenticated key handoff), verified over a real network in the dev
    VM. See `docs/agent-protocol.md`'s "Pairing protocol" section.
  - `setup-wizard/first-boot/` — the first-boot hook (issue #27): a
    systemd unit chained `After=omarchy-provision-owner.service`,
    `Before=display-manager.service` (Omarchy 4 has no official first-boot
    extension point — issue #26's research found the flow is one
    monolithic script with no hook/drop-in point, so this integrates via
    unit ordering instead), plus the `gum`-based parent-facing form
    (name/tier/language) that drives the bootstrap script. Also fixes a
    found-along-the-way gap: Omarchy's own first-boot drops SDDM autologin
    after the first boot on unencrypted installs, which would silently
    reintroduce a reachable login prompt for the mini tier — the wizard
    now keeps autologin permanent instead. Also now invokes pairing itself
    (issues #22/#23 wiring): after bootstrap, runs `sudo -u <child_user>
    omarchy-kids-pairing serve`, opening/closing the pairing port's UFW
    rule around the call (the gap noted in `docs/agent-protocol.md` — no
    sudoers workaround needed, the wizard already runs as root). `serve`
    prints its pairing code/QR straight to tty1, so no extra UI was
    needed. Deliberately best-effort: Control Center doesn't exist yet, so
    a skipped/timed-out/failed pairing just logs a warning and setup still
    completes — re-pairing later is a manual re-run (#25, retry UX, is
    intentionally not built here). Verified so far in the dev VM: account
    detection, tier discovery, the UFW open/close functions, and a real
    `serve`/`pair` round trip over `sudo -u <child_user>` (correctly
    installed the key with correct ownership; `SIGINT` mid-`serve` exits
    non-zero as expected). Not yet verified: the `gum` prompts end-to-end,
    or the systemd unit against a real `omarchy-provision-owner.service`
    run (needs a fresh deferred-provisioning install). See its README.
  - Not yet done: failed-pairing retry UX (#25); multi-child reuse (#28,
    cross-machine, i.e. reuse between siblings' separate machines). A
    related but separate question — multiple children sharing one
    machine — was raised and intentionally deferred for now (SDDM
    autologin is single-account machine-wide, the real blocker); see the
    vault note's "Offene Frage: mehrere Kinder auf EINEM Rechner" tracking
    entry.
- `control/` now has a real first slice, not just a stub: pairing.
  Deliberate architecture decision (2026-08-29) — Control Center is C++,
  but rather than reimplementing the already-verified SPAKE2 protocol in
  C++, `control/gui/`'s `PairingDialog` drives `omarchy-kids-pairing pair`
  as a subprocess (same "shell out to a trusted binary" pattern already
  planned for `ssh`), reading its fingerprint line for a real parent
  confirmation and a final `PAIR_RESULT` JSON line for the outcome — the
  reference CLI's own auto-confirm was always meant as a stand-in for
  exactly this dialog, now replaced. `control/core/`'s `HostRegistry`
  persists paired children as TOML at
  `~/.config/omarchy-kids-control/hosts.toml` (`tomlplusplus`, matches the
  vault note's own data-model sketch). `MainWindow` is a minimal shell (a
  host list plus "Pair a new child...") — the real dashboard (usage stats,
  app unlocks, tier changes), the TUI frontend, and the headless polling
  mode are all still not built. Verified with a real, complete round trip
  against the dev VM: GUI → subprocess → SPAKE2 exchange → parent-confirmed
  fingerprint → key installed in the child's `authorized_keys` → SSH login
  through that key correctly restricted to `omarchy-kids-agent`. Two real
  bugs found and fixed during that verification: a failed `QProcess::start`
  (e.g. the binary missing from PATH) went unhandled and silently hung the
  dialog forever (now handled via `errorOccurred`); retrying after a failed
  attempt for the same child name collided with the stale key file the
  first attempt had already written (now cleaned up before each attempt),
  and a stale process from an abandoned attempt was never terminated,
  risking a delayed reply landing on whatever attempt started next (now
  killed before starting a new one). See `docs/agent-protocol.md`'s
  "Pairing protocol" section for the `pair` CLI's own changes (interactive
  confirmation replacing auto-confirm, `--yes` for scripting, host/port now
  printed by `serve` so the manual-entry path is actually usable).
  `quickshell-plugin/` is still a stub/skeleton — no real logic yet.
- Not yet done: app installation as part of the package (nothing in the
  current line-up is installed by the package yet), the `omarkid-gcompris`
  fork itself, app-wrapper/time-tracking integration, locale implementation
  (concept is written, not built).

## Dev environment

- **Parent computer**: the developer's own Omarchy machine, native, no VM.
- **Child computer**: a libvirt/QEMU VM (`omarchy-kids-child`) on the same
  machine. **Full step-by-step for building one from scratch — including
  the firewall rules you WILL hit and the SSH bootstrap — is in
  [`docs/dev-vm-setup.md`](docs/dev-vm-setup.md). Read that before trying
  to stand up or debug a dev VM instead of rediscovering the same UFW/SSH
  issues.**
- `scripts/vm-type-de.sh` — types text into the VM console via
  `virsh send-key` for when SSH isn't up yet (German/QWERTZ keyboard
  layout mapping — see its header comment before using it against a
  different-layout guest).
- Iterating on `tiers/`: `rsync` the folder to the VM, then run
  `omarchy-kids-set-tier <tier>` there — exact commands in the "Quick
  reference" section at the bottom of `docs/dev-vm-setup.md`.

## Non-obvious things worth knowing before touching this codebase

- **Omarchy ships UFW active by default**, blocking inbound SSH — not just
  a `command=`-key problem. Handled: the agent package's `post_install`
  runs `ufw allow ssh` (see `docs/agent-protocol.md`). Still open: the
  pairing listener's own port (default 7420) has no UFW rule at all yet —
  nothing opens/closes it around a `omarchy-kids-pairing serve` call (see
  `docs/agent-protocol.md`'s "Pairing protocol" section).
- **SSH-invoked commands run outside the graphical session** (no Wayland/
  D-Bus env vars) — this is *why* the architecture splits `agent` (thin SSH
  receiver) from `agentd` (session-resident daemon that actually touches
  the live desktop). Don't have the SSH-reachable `agent` try to run
  session-affecting commands directly.
- **Quickshell third-party overlay plugins are opt-in** via
  `~/.config/omarchy/shell.json`'s `plugins[]` array — a plugin directory
  alone does nothing. `omarchy-kids-set-tier` handles this.
- **Kiosk lockdown is a UI-layer thing only, unless you also close the OS
  escape hatches.** Hiding apps from the launcher doesn't stop a VT switch
  (`Ctrl+Alt+F2`) to a raw login shell on the same account. Hence the
  getty-masking in `omarchy-kids-set-tier` for the mini tier. Same principle
  bit again on 2026-08-29: overriding Hyprland's keybindings doesn't touch the
  Omarchy top bar's own menu icon, which still opens the normal Omarchy menu
  by mouse/touch — `omarchy-kids-set-tier` now hides the bar for tier mini
  too (see `tiers/README.md`).
- **Franchise-themed content (Paw Patrol, Peppa Pig, Bluey, ...) is
  deliberately out of scope for now** — own generic themes only; see
  `Omarchy Kids - Themes` for the licensing reasoning and the later plan to
  approach rights holders directly.
- Root SSH access to the dev VM (see `docs/dev-vm-setup.md` §8) is dev-only
  convenience, unrelated to the production agent's `command=`-restricted
  key design. Don't let dev-VM shortcuts leak into the real architecture.

## Git

Commit only when explicitly asked. This repo had `website/` content
appear mid-session from a separate, parallel session working on it
independently — if you see files you didn't create, they may be someone
else's in-progress work; check before touching.
