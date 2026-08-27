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

**Only tier 5-7 is being built right now** — explicitly deprioritized the
other tiers (8-10, 11-13, 14-16) until 5-7 is solid. Don't start scaffolding
other tiers unless asked.

## Status (as of the last session)

- Monorepo scaffold in place: `agent/` (Rust workspace, builds), `control/`
  (CMake/Qt6, builds), `tiers/`, `quickshell-plugin/`, `setup-wizard/`,
  `docs/` — see each folder's README for stack/status.
- `tiers/5-7/` is a working end-to-end kiosk, verified in the dev VM:
  - `theme/` — "Sternenreise" (own space artwork, not a licensed franchise)
  - `launcher/omarchy-kids.launcher/` — fullscreen Quickshell overlay plugin,
    icon-only grid (GCompris, Tux Paint, Blinken), launches via `gtk-launch`
  - `hypr/hyprland.lua` — full Hyprland config replacement: every default
    binding off, only `SUPER+SPACE` survives (→ the kiosk launcher)
  - `omarchy-kids-set-tier` — applies all of the above, plus masks
    `getty@tty2-6` (VT-switch lockdown) for this tier
- `agent/`, `control/`, `quickshell-plugin/`, `setup-wizard/` are stubs/skeletons only — no real logic yet.
- Not yet done: app installation as part of the package (GCompris/Tux
  Paint/Blinken are manually installed in the dev VM right now — Tux Paint
  is AUR-only, see the packaging note in the Altersstufe-5-7 vault note),
  app-wrapper/time-tracking integration, real SSH pairing/agent, locale
  implementation (concept is written, not built).

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

- **Omarchy ships UFW active by default**, blocking inbound SSH. The real
  setup-wizard has to open the agent's SSH port explicitly — it's not just
  a `command=`-key problem. (`Omarchy Kids - Implementierung Agent` vault note.)
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
  getty-masking in `omarchy-kids-set-tier` for 5-7.
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
