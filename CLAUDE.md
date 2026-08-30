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
- `agent/` (issues #1-#15, plus #10 and #25 from the other two project
  boards — see below) is implemented, not a stub: `agent`/`agentd`/
  `omarchy-kids-run`/`omarchy-kids-override-helper`/
  `omarchy-kids-repair-helper` — protocol, time budgets, PIN override,
  production re-pairing trigger, packaging. See `docs/agent-protocol.md`.
  Found and fixed along the way (2026-08-29): the pairing-installed
  `authorized_keys` entry's `command=` restriction execs `omarchy-kids-agent`
  with zero argv, ignoring `$SSH_ORIGINAL_COMMAND` — every remote command
  (not just Control Center's new dashboard below) silently collapsed to a
  usage error until `agent/src/main.rs` gained `effective_args()` to read it
  back out. Prior pairing verification never caught this because it only
  ever tested the SSH *login*, never a real remote command.
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
    completes — re-pairing later is now `omarchy-kids-agent repair-pairing`
    (#25, see below), not a manual re-run of the wizard. Verified so far in
    the dev VM: account
    detection, tier discovery, the UFW open/close functions, and a real
    `serve`/`pair` round trip over `sudo -u <child_user>` (correctly
    installed the key with correct ownership; `SIGINT` mid-`serve` exits
    non-zero as expected). Not yet verified: the `gum` prompts end-to-end,
    or the systemd unit against a real `omarchy-provision-owner.service`
    run (needs a fresh deferred-provisioning install). See its README.
  - Failed-pairing retry UX (#25) is now resolved: mDNS+QR stay live for the
    whole pairing window (no sequential fallback needed) and a
    skipped/timed-out/failed attempt just logs a warning, per above. The one
    real gap it surfaced — re-triggering pairing later on a *production*
    machine with no reachable shell — is closed by the new
    `omarchy-kids-agent repair-pairing` (see `agent/` above). Multi-child
    reuse (#28, cross-machine, i.e. reuse between siblings' separate
    machines) was decided: the wizard runs fully independently per machine,
    no shared defaults for now — revisit once Control Center holds
    family-level state worth sharing. A related but separate question —
    multiple children sharing one machine — was raised and intentionally
    deferred for now (SDDM autologin is single-account machine-wide, the
    real blocker); see the vault note's "Offene Frage: mehrere Kinder auf
    EINEM Rechner" tracking entry.
- `control/` now has real first slices, not just a stub: pairing and a
  dashboard. Deliberate architecture decision (2026-08-29) — Control Center is C++,
  but rather than reimplementing the already-verified SPAKE2 protocol in
  C++, `control/gui/`'s `PairingDialog` drives `omarchy-kids-pairing pair`
  as a subprocess (same "shell out to a trusted binary" pattern already
  planned for `ssh`), reading its fingerprint line for a real parent
  confirmation and a final `PAIR_RESULT` JSON line for the outcome — the
  reference CLI's own auto-confirm was always meant as a stand-in for
  exactly this dialog, now replaced. `control/core/`'s `HostRegistry`
  persists paired children as TOML at
  `~/.config/omarchy-kids-control/hosts.toml` (`tomlplusplus`, matches the
  vault note's own data-model sketch). `MainWindow` grew a first real
  dashboard (2026-08-29, closes agent-project issue #10's cross-host half):
  selecting a paired child polls it over SSH (new Qt-free
  `control/core/AgentClient`, `omarchy-kids-agent status`/`report --json`,
  execvp'd directly — no shell, since host/key values can come from LAN
  pairing data) and shows tier/budget/unlocked-apps status plus a
  security-events list; severe events fire a local `notify-send` on the
  *parent's* computer. Polling runs off the GUI thread via `QThreadPool` +
  a lifetime-safe `QMetaObject::invokeMethod` hop back. Still not built:
  app-unlock/tier-switch controls, usage-stat charts, the TUI frontend, and
  a headless polling mode (today only the currently-selected host is
  polled, so a severe event on an unselected child won't notify until it's
  selected again) — see `docs/agent-protocol.md`'s "Control Center's
  dashboard" section. Verified with a real, complete round trip
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
- **Full real end-to-end verification done (2026-08-29, closes setup-wizard
  issue #29):** not a stand-in — a fresh Omarchy VM, deferred-provisioning
  install, `omarchy-kids-*` binaries/scripts/units injected onto its disk
  offline (`qemu-nbd`, before its first real boot — `setup-wizard`/`tiers`
  aren't packaged yet), then driven through the actual first-boot chain via
  `virsh screenshot`/`send-key`: `omarchy-provision-owner.service` → our
  chained `omarchy-kids-setup-wizard.service` (gum form, real tty1) →
  bootstrap → `run_pairing`'s pairing window → paired from the host with
  `omarchy-kids-pairing pair` → the real Control Center GUI, launched on
  the parent's own desktop, showing the child online with live status.
  Found and fixed two real gaps this surfaced: a binary-path mismatch
  (`command=` hardcodes `/usr/bin/omarchy-kids-agent`; the binaries were
  found at `/usr/local/bin` first) and a genuine pairing-protocol gap — the
  child's username was never transmitted at all, so Control Center had no
  way to know which account's `authorized_keys` held the paired key
  (`AgentClient` was building `ssh host` instead of `ssh user@host`).
  `SecurePayload::Confirm`, `PAIR_RESULT`, `HostEntry`/`hosts.toml`, and
  `PairingDialog` all now carry `username`. See `docs/agent-protocol.md`'s
  "End-to-end verification" section.
- **Issue #30 fixed (2026-08-30):** the mini-tier kiosk was showing Omarchy's
  generic first-login onboarding — found this covers more than the two
  notifications originally reported: `omarchy-provision-first-run`'s whole
  sequence is adult-facing prompts (System-Update, Learn-Keybindings,
  Setup-Wi-Fi, plus post-update invitations for Voxtype/fingerprint/default-
  agent setup) except for four silent steps, with no per-step toggle — only
  one all-or-nothing done-marker. New `tiers/mini/hypr/autostart.lua`,
  installed by `omarchy-kids-set-tier` alongside `hyprland.lua`: marks that
  done-marker itself before the child's first graphical login (so none of
  Omarchy's prompts ever fire) and replicates only the four worth-keeping
  silent steps by calling Omarchy's own scripts directly, in the real
  Hyprland session where they actually work (unlike the wizard's pre-session
  tty1 context). See `tiers/README.md`'s "Status" section.
- **CI added (2026-08-30):** first `.github/workflows/ci.yml` — `agent/`
  (cargo build/clippy/test, verified clean), `control/` (CMake/Ninja/Qt6/
  tomlplusplus build), and shellcheck/`luac` lint over the shell scripts and
  tiers' Hyprland Lua configs. All fast, no VM. Separately, the slow part of
  local dev-loop iteration — reinstalling the dev VM from ISO and manually
  replaying the pairing round trip via `virsh screenshot`/`send-key` — now
  has two faster stand-ins in `scripts/`: `vm-snapshot.sh` (internal qcow2
  snapshot/revert, so a test pass doesn't need a fresh install) and
  `vm-pairing-smoke-test.sh` (scripts the pairing protocol round trip via
  the reference `pair --yes` CLI and checks the machine-readable
  `PAIR_RESULT:` line instead of eyeballing screenshots). Found along the
  way: the dev VM's default raw-format NVRAM (from `virt-install --boot
  uefi`) rejects `virsh snapshot-create-as` outright — `docs/dev-vm-setup.md`
  §4 now requests qcow2 NVRAM for new VMs; migrating the existing VM is
  documented there but not verified this session. Neither script replaces
  the full from-ISO end-to-end test (`docs/agent-protocol.md`'s "End-to-end
  verification") — console-form keystroke automation is too fragile to lean
  on exclusively — they speed up iterating on the pairing protocol itself.
  See `docs/dev-vm-setup.md`'s "Fast iteration" section.
- **`quickshell-plugin/` is no longer a stub (2026-08-30):** the parent-
  computer headerbar plugin (`omarchy-kids.control/`, kind `bar-widget`) is
  implemented — a bar icon that turns the bar's "needs attention" color
  when any paired child is offline, and a click-to-open popup (`qs.Ui`'s
  `Panel`/`KeyboardPanel`, same mechanism as Omarchy's own Dropbox/Clock bar
  widgets) listing every paired child with an online/offline dot plus an
  "Open Control Center" row. It never speaks SSH itself, per the trust-
  boundary decision in `Omarchy Kids - Implementierung Control Center` —
  it only reads `~/.config/omarchy-kids-control/status-cache.json` via a
  `FileView`. That cache is written by a new headless poll mode,
  `omarchy-kids-control --poll` (`control/core/`'s new `status_cache`/
  `poll_runner`, dispatched before `QApplication` in `gui/src/main.cpp` so
  it needs no display), meant to run periodically via a new
  `control/packaging/systemd/omarchy-kids-control-poll.timer` — not
  installed/enabled by anything yet, so today the cache is only as fresh as
  the last manual `--poll` run. Verified for real: a real poll against the
  already-paired dev-VM host wrote a real cache entry, and the plugin
  rendered it live on the parent's own desktop (`quickshell-plugin/
  install-dev.sh`, a dev-only installer — `control/` itself still has no
  PKGBUILD). Also added: `control/`'s first test infrastructure (it had
  none before, not even for `HostRegistry`/`AgentClient`) — a GTest suite
  at `control/tests/` covering `StatusCache` (JSON escaping/round-trip,
  full-replace-on-each-write) and `HostRegistry` (persist/reload, re-pair-
  updates-in-place, malformed-file handling), wired into CI via `ctest`.
  `gui/`'s Qt widgets and `AgentClient`'s SSH calls remain untested beyond
  the existing end-to-end VM verification — a real gap, left for later.
- **Control Center dashboard grew an app-unlock control (2026-08-30).**
  `MainWindow` now has a "desktop id + minutes + Unlock" row next to the
  status panel — sends `omarchy-kids-agent unlock <app> --minutes N --json`
  over the same `AgentClient`/`QThreadPool` pattern the existing status/
  report polling already uses, then immediately re-polls so the status line
  reflects the new unlock without waiting for the next timer tick. A
  tier-switch control was deliberately *not* added alongside it: only the
  "mini" tier exists at all right now (see "Current focus"), so there's
  nothing else to switch to yet — revisit once a second tier exists.
  Usage-stat charts are still missing. Verified for real end-to-end against
  a freshly rebuilt dev VM (see below): clicked Unlock for
  `org.kde.gcompris` in the live GUI, then confirmed over a separate SSH
  session that agentd's `status --json` actually listed it in
  `unlocked_apps` — not just that the GUI showed something.
- **Dev VM rebuilt from scratch (2026-08-30) — the old one was stuck, not
  salvageable.** It was found sitting at a UEFI boot device menu, never
  reaching the OS, and `vm-snapshot.sh list` had nothing to revert to (it
  was apparently never migrated to qcow2 NVRAM despite `docs/dev-vm-setup.md`
  already documenting that step) — matches the doc's own framing of this VM
  as throwaway, so it was undefined and recreated via `virt-install` per
  `docs/dev-vm-setup.md` §4 rather than debugged. New gotcha found doing
  this: `--boot uefi,nvram.templateFormat=qcow2` (the doc's current §4
  command) now fails outright on this host — `ERROR operation failed:
  Unable to find 'efi' firmware that is compatible with the current
  configuration`. `virsh domcapabilities` reports `<varstore supported='no'/>`
  for this host's libvirt/QEMU/edk2-ovmf combination, so qcow2-templated
  NVRAM isn't available here at all right now, not just unmigrated on the
  old VM. Worked around at the time by dropping `nvram.templateFormat=qcow2`
  (plain `--boot uefi`, default raw NVRAM) to get unblocked; **root-caused
  and properly fixed later the same day** — see the next bullet — so this
  workaround is no longer needed for new VMs. New VM: same username (`devchild`), new IP
  `192.168.122.126` (updated in `~/.ssh/config`), re-paired fresh (new
  `control/` host entry, replacing the stale one from the old VM). Also
  found and fixed a real bug while driving the reinstall:
  `scripts/vm-type-de.sh` called `virsh send-key` with no `-c
  qemu:///system`, so every keystroke silently went nowhere (defaulted to
  `qemu:///session`, which doesn't see a system-connection domain) — the
  script's own `>/dev/null 2>&1` swallowed the resulting "failed to get
  domain" error, so this looked like nothing was happening rather than an
  outright failure. Now fixed.
- **qcow2-NVRAM/snapshot support actually root-caused and fixed
  (2026-08-30).** As of libvirt ~10.10, a domain's NVRAM `format` must
  exactly match its `templateFormat` — no auto-conversion at start time —
  and Arch's `edk2-ovmf` only ships a raw template, so the simple `--boot
  uefi,nvram.templateFormat=qcow2` form (firmware *auto-selection*) can
  never work here: no installed firmware descriptor declares a qcow2
  template. Fixed by converting `OVMF_VARS.4m.fd` to qcow2 once
  (`/var/lib/libvirt/boot/OVMF_VARS.4m.qcow2`) and pointing new VMs at it
  explicitly via `--boot loader=...,nvram.template=...,nvram.templateFormat=qcow2`
  (bypassing auto-selection entirely) — `docs/dev-vm-setup.md` §4 updated.
  Migrated the current dev VM in place the same way (existing raw
  `_VARS.fd` converted, domain redefined with hand-edited XML — virt-install/
  virt-xml's `--boot` CLI has no property for the NVRAM `format=` attribute
  at all, so this step can't be done through their CLI, only by editing the
  XML directly). Verified with a real save→modify→revert round trip: created
  a marker file, snapshotted, changed state further, reverted, confirmed the
  marker was gone and agentd/pairing survived intact. Also fixed
  `scripts/vm-snapshot.sh` alongside this — it called `sudo virsh`, which
  fails silently (no visible error) in a non-interactive context, so an
  early revert attempt looked like it worked but had done nothing. Full
  root cause and the exact migration steps are in `docs/dev-vm-setup.md`'s
  "Known gotchas" section.
- **Dev VM recreation + agent deployment + pairing fully scripted, via
  Omarchy's own unattended-install mechanism, not console automation
  (2026-08-30/31).** New `scripts/vm-recreate.sh` drives the *entire* VM
  lifecycle with zero keystrokes sent at all: `scripts/vm-cidata-build.sh`
  builds a `cidata`-labeled ISO (the cloud-init NoCloud convention Omarchy's
  installer looks for — see
  [the manual](https://omarchy.org/manual/unattended-installs/) and the
  real source in
  [omacom-io/omarchy-iso](https://github.com/omacom-io/omarchy-iso))
  carrying `user_configuration.json`/`user_credentials.json` (the exact
  JSON the interactive wizard itself writes — reproduced by hand from that
  repo's `configurator` script/`orchestrator/context.py`, including its
  partition-size arithmetic for a fixed 40G disk) plus an `authorized_keys`
  file. Attaching it makes the installer skip its interactive wizard
  *entirely* and, because `authorized_keys` is present, enables `sshd` and
  opens `ufw` on its own before rebooting — so `vm-recreate.sh` only waits
  (poll `domstate`, then a DHCP lease, then SSH), never types anything. A
  fixed, dev-only account (`devchild`/`omarchy-kids-dev`) is baked into
  that JSON — not a secret worth protecting, see prior reasoning.
  **Superseded approach, kept here as a record:** two earlier attempts this
  session drove the interactive wizard via blind `virsh send-key` (first
  fixed sleeps, then a screenshot-stability-based `settle()` function) —
  both got desynced from the actual screen under real host load and
  corrupted the account-setup sequence in ways only discovered deep into
  the run (an "Enter" landing on a screen that hadn't rendered yet, or on
  one that silently never changed because the keypress was dropped, which
  a stability check can't distinguish from "already settled"). The cidata
  approach has nothing to time against a screen for, so there's nothing
  left to desync. `scripts/vm-type-us.sh` (a US-layout companion to
  `vm-type-de.sh`) survives as a debugging tool, not a dependency of
  `vm-recreate.sh` anymore.
  New `scripts/vm-deploy-agent.sh` (build + scp + install to `/usr/bin` +
  systemd unit) and `scripts/vm-pair-for-dashboard.sh` (opens the pairing
  UFW rule, pairs, prints a ready `hosts.toml` block, closes the rule
  again) close the remaining gaps. Real gotcha found deploying to a truly
  unattended VM: nobody is ever logged into the graphical session, so
  `agentd`'s `PartOf=graphical-session.target` unit got torn down the
  moment the enabling `ssh -tt` session closed (systemd-logind stops
  session-bound units once a user's last session ends) — fixed with
  `loginctl enable-linger devchild` in `vm-deploy-agent.sh`. Key technique
  for the scripted `sudo` calls generally: `ssh -tt host "echo
  \"$PASSWORD\" | sudo -S cmd"` — plain `sudo` over non-interactive SSH
  refuses to read a password at all (no `sshpass`/`expect` needed). Also:
  `virsh vol-create-as`/`vol-upload` into a libvirt storage pool needs no
  `sudo` at all (unlike `cp` into `/var/lib/libvirt/...` directly) — used
  to get both the cidata volume and (in an earlier iteration) an SSH-key
  ISO onto the host without ever needing a human for that step. Verified
  with a real, complete run: fresh VM → unattended install → SSH (sshd/ufw/
  key already configured by the installer) → agent binaries → agentd
  running (surviving session teardown) → paired → `hosts.toml` entry →
  snapshot saved. Full walkthrough and rationale in
  `docs/dev-vm-setup.md`'s "Fast path" section at the top. Deliberately
  kept tier application (`omarchy-kids-set-tier`) as a separate step/script
  (`scripts/vm-apply-tier.sh`), not folded into `vm-recreate.sh` — two
  distinct test scenarios need two different starting snapshots:
  `fresh-boot` (bare, paired, no tier — for testing onboarding/pairing
  itself, repeatable via `vm-pair-for-dashboard.sh`) vs. `kiosk-ready`
  (tier applied — for testing features on an already-set-up kids computer).
  Both snapshots verified for real on the same VM (VT lockdown actually
  `masked`, not just `disabled`; kiosk launcher plugin present; Hyprland's
  `SUPER + SPACE` kiosk binding confirmed in place).
- Not yet done: app installation as part of the package (nothing in the
  current line-up is installed by the package yet), the `omarkid-gcompris`
  fork itself, app-wrapper/time-tracking integration, locale implementation
  (concept is written, not built — a maintainer note from 2026-08-29 also
  flags that the setup wizard's language prompt should move to *first*,
  once i18n is actually built, so later prompts render in it).

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
