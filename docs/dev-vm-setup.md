# Dev VM setup: fresh Omarchy child computer

Step-by-step for standing up a throwaway Omarchy VM on your own machine to
develop/test `tiers/`, the launcher plugin, and (later) the agent/control
center against. Written from doing this once on a Linux laptop running
Omarchy natively as the host; nothing below is specific to that hardware.

Everything here is dev-only. None of it — the root SSH access, the ISO
handling, the blind-keyboard-typing fallback — is part of the production
setup-wizard/pairing flow that ships to an actual child computer.

## Fast path: fully automated recreation

Once §1-2 below are done once per host, recreating the child VM from
scratch no longer needs sections 3-7 done by hand at all —
`scripts/vm-recreate.sh` drives the whole thing using Omarchy's own
**unattended-install mechanism**
([manual](https://omarchy.org/manual/unattended-installs/), real source in
[omacom-io/omarchy-iso](https://github.com/omacom-io/omarchy-iso)): a
second, `cidata`-labeled ISO (built by `scripts/vm-cidata-build.sh`, the
cloud-init NoCloud label libvirt/Proxmox/Packer all already know how to
attach) carries `user_configuration.json`/`user_credentials.json` — the
exact JSON the installer's own interactive wizard would write — plus an
`authorized_keys` file. Its presence makes the installer skip its
interactive wizard **entirely** (no keystrokes needed at all, not even
"press Return to start") and, because `authorized_keys` is present, the
installer enables `sshd` and opens `ufw` for SSH on its own before ever
rebooting. A **fixed, dev-only account** (`devchild` / `omarchy-kids-dev`,
English (US) keyboard, disk encryption disabled) is baked into that JSON.
This isn't a secret worth protecting: the VM lives on an isolated libvirt
NAT network never reachable from outside the host, and is throwaway by
design — same idea as Vagrant boxes shipping a well-known default keypair.
Verified end-to-end 2026-08-31.

```bash
scripts/vm-recreate.sh                       # ~5 minutes, no interaction
scripts/vm-deploy-agent.sh <ip>               # IP is printed at the end
scripts/vm-pair-for-dashboard.sh <ip> Testkind        # prints a hosts.toml block
scripts/vm-snapshot.sh save fresh-boot                # so the next reset is instant
```

**Two snapshots, two test scenarios** — deliberately kept as separate steps
rather than one script, since they answer different questions:

- **`fresh-boot`** (above): a bare, paired child machine with `agentd`
  running but no tier applied yet — the right starting point for testing
  *onboarding itself* (re-run `vm-pair-for-dashboard.sh` against it
  repeatedly to exercise the pairing flow without redoing the OS install).
- **`kiosk-ready`**: `fresh-boot` plus `omarchy-kids-set-tier` actually
  applied — the right starting point for testing *features on an already
  set-up kids computer* (kiosk launcher, VT lockdown, theme all live).

```bash
scripts/vm-snapshot.sh restore fresh-boot    # back to bare+paired
scripts/vm-apply-tier.sh <ip> mini
scripts/vm-snapshot.sh save kiosk-ready      # back to fully set up
```

**Why not console keystroke automation** (`virsh send-key` against the
interactive wizard, this section's approach before 2026-08-31): two
separate attempts got desynced from the actual screen under real host load
(fixed sleeps, then a screenshot-stability check — both corrupted the
account-setup sequence in ways only discovered much later, deep into the
run; see git history on this file for the gory details if curious). The
cidata approach doesn't time anything against a screen at all, so there's
nothing left to desync from. `scripts/vm-type-us.sh`/`vm-type-de.sh` are
kept around as debugging tools (e.g. to poke around inside a stuck
install) and for §5-7's fully-manual fallback path below, not because
`vm-recreate.sh` still needs them.

**A real gotcha this surfaced**: a freshly-`vm-recreate.sh`'d VM has nobody
actually logged into the graphical session (unattended installs skip that
entirely), so `agentd`'s `PartOf=graphical-session.target` unit gets torn
down the moment whichever `ssh -tt` session first enabled it closes —
systemd-logind stops session-bound units once a user's last session ends,
and an unattended VM's "last session" is always some short-lived SSH
connection. Fixed in `vm-deploy-agent.sh` with `loginctl enable-linger
devchild`, which keeps the user's systemd instance (and anything bound to
it) running independent of login sessions — the same mechanism production
machines need anyway for `agentd` to survive a graphical logout.

## 1. Host packages + libvirt

```bash
sudo pacman -S --needed libvirt virt-manager virt-install dnsmasq qemu-desktop
sudo systemctl enable --now libvirtd.service
sudo usermod -aG libvirt "$USER"   # takes effect on next login, or: newgrp libvirt

sudo virsh net-start default
sudo virsh net-autostart default

# Only needed if no storage pool exists yet (`sudo virsh pool-list --all`):
sudo virsh pool-define-as default dir --target /var/lib/libvirt/images
sudo virsh pool-start default
sudo virsh pool-autostart default
```

## 2. Host firewall — do this even if you think you don't need it

If the host runs UFW (`sudo ufw status`), its default-deny policy blocks the
VM in **two different ways** that look unrelated when you hit them:

```bash
# Without this: the VM never gets a DHCP lease (dnsmasq's replies never
# reach it) and you can't SSH into it either — both are host-INPUT traffic.
sudo ufw allow in on virbr0 comment 'libvirt default NAT network'

# Without this: the VM gets an IP and you can SSH in, but it has no
# internet access — pacman/curl/etc. from inside the VM just time out.
# This is host-FORWARD (routed/NAT) traffic, a separate UFW policy.
sudo ufw route allow in on virbr0 comment 'libvirt default NAT network - forwarded traffic'
```

Both rules are scoped to the isolated `virbr0` bridge (`192.168.122.0/24`),
not exposed to any external network.

## 3. Get the Omarchy ISO

```bash
curl -L --fail -o ~/Downloads/omarchy-latest.iso https://iso.omarchy.org/omarchy-4.0.1.iso
sudo mkdir -p /var/lib/libvirt/boot
sudo mv ~/Downloads/omarchy-latest.iso /var/lib/libvirt/boot/omarchy.iso
sudo chmod 0644 /var/lib/libvirt/boot/omarchy.iso
```

Check [omarchy.org](https://omarchy.org) for the current version — the ISO
URL is versioned (`omarchy-X.Y.Z.iso`). Download to `~/Downloads` first
rather than straight into `/var/lib/libvirt/boot` with `sudo curl`: if your
`sudo` needs interactive auth (password prompt, biometric, whatever), doing
the download itself as root over a background/non-interactive shell can
fail with `sudo: a terminal is required` — downloading as your normal user
and moving the finished file with one `sudo mv` sidesteps that regardless
of how your `sudo` is set up.

## 4. Create the VM

One-time per host: Arch's `edk2-ovmf` package only ships a **raw** NVRAM
template (`/usr/share/edk2/x64/OVMF_VARS.4m.fd`) — there's no qcow2 variant
to point at, unlike Fedora/Debian. Convert one yourself, once, and reuse it
for every VM you create from now on:

```bash
sudo qemu-img convert -O qcow2 \
  /usr/share/edk2/x64/OVMF_VARS.4m.fd /var/lib/libvirt/boot/OVMF_VARS.4m.qcow2
sudo chmod 0644 /var/lib/libvirt/boot/OVMF_VARS.4m.qcow2
```

Then:

```bash
sudo virt-install \
  --name omarchy-kids-child \
  --memory 6144 \
  --vcpus 4 \
  --cpu host-passthrough \
  --disk pool=default,size=40,format=qcow2,bus=virtio \
  --cdrom /var/lib/libvirt/boot/omarchy.iso \
  --os-variant archlinux \
  --network network=default,model=virtio \
  --graphics spice \
  --video virtio \
  --boot loader=/usr/share/edk2/x64/OVMF_CODE.4m.fd,loader.readonly=yes,loader.type=pflash,nvram.template=/var/lib/libvirt/boot/OVMF_VARS.4m.qcow2,nvram.templateFormat=qcow2 \
  --features smm=off \
  --noautoconsole \
  --wait -1
```

Adjust `--memory`/`--vcpus` to the host's headroom (`free -h`) — 6GB/4 vCPUs
assumes an 8-16GB host that's also running other things. UEFI without
secure boot and `smm=off` matches Omarchy's own requirement to have Secure
Boot/TPM disabled. The explicit `loader=`/`nvram.template=` form (rather
than the simpler `--boot uefi,nvram.templateFormat=qcow2`) is required —
see "Known gotchas" for why the simpler form doesn't work on this host at
all, not just on VMs predating this doc.

## 5. Install (interactive — this part you drive yourself)

Open `virt-manager`, connect to `omarchy-kids-child`'s console.

- **At the keyboard-layout screen, press Ctrl+C.** This defers personal
  config (username, password, keyboard layout) to first boot instead of
  asking now — the same "deferred first-boot provisioning" path the real
  `omarchy-kids-setup-wizard` is meant to hook into, so exercising it here
  is deliberate, not just a shortcut.
- Pick the target disk, confirm. Install runs on its own, well under 5
  minutes.
- Reboot when done. First boot asks for keyboard layout, username,
  password — pick whatever; note the username, you'll need it for SSH.
  **If you pick a German keyboard layout, see the note in §7** before
  typing anything you need to be byte-exact (like a pasted SSH key).

## 6. Fix networking (almost always needed)

Fresh boot usually leaves the VM's network disconnected — Omarchy shows a
"Setup Wi-Fi" hint even for the virtio ethernet device. From the VM's own
terminal (`SUPER+Return`):

```bash
nmcli device status              # find the ethernet device, e.g. enp1s0
nmcli device connect enp1s0
```

If that fails with "IP configuration could not be reserved" — that's the
host firewall from §2, not the VM. Go fix that first.

Then enable SSH **inside the VM** — Omarchy ships with UFW active by
default and SSH not allowed, on top of `sshd` itself being disabled:

```bash
sudo systemctl enable --now sshd
sudo ufw allow ssh
```

Get the VM's IP from the host: `sudo virsh net-dhcp-leases default`.

## 7. SSH access from the host

Generate a dev keypair once — reuse it across VM rebuilds, it's not
tied to any one VM:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/omarchy_kids_dev -N "" -C "omarchy-kids-dev"
```

Add to `~/.ssh/config` (adjust the IP each time you recreate the VM):

```
Host omarchy-kids-child
    HostName 192.168.122.109
    User <the username you picked at first boot>
    IdentityFile ~/.ssh/omarchy_kids_dev
    StrictHostKeyChecking accept-new
```

**Getting the public key into the VM's `~/.ssh/authorized_keys`:** if you
already have another way to reach the VM (shared clipboard, a mounted
volume), just paste it. If not — no clipboard, no SSH yet, only the
console — you have two options:

- **Preferred: mount a small ISO with the key, no typing at all.**
  ```bash
  mkdir -p /tmp/keydata && cp ~/.ssh/omarchy_kids_dev.pub /tmp/keydata/authorized_keys
  genisoimage -output /tmp/key.iso -volid KEYDATA -joliet -rock /tmp/keydata/
  sudo cp /tmp/key.iso /var/lib/libvirt/boot/
  sudo virsh change-media omarchy-kids-child sda --source /var/lib/libvirt/boot/key.iso --insert --live
  ```
  Then in the VM: `sudo mkdir -p ~/.ssh && sudo mount /dev/sr0 /mnt && sudo cp /mnt/authorized_keys ~/.ssh/ && sudo chown $(whoami):$(whoami) ~/.ssh/authorized_keys && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && sudo umount /mnt`
  (`sda` is whatever `virsh domblklist omarchy-kids-child` shows as the CD-ROM target — usually the same slot the install ISO used.)

- **Fallback: type it via `virsh send-key`.** Only do this if the ISO
  route isn't available. `scripts/vm-type-de.sh <domain> "<text>"` in this
  repo drives the console character-by-character — **it assumes a German
  (QWERTZ) keyboard layout in the guest** (Y/Z swapped, different
  punctuation scancodes; see the comment header in the script). A base64
  SSH key blob retyped this way got one character subtly wrong here on the
  first attempt (looked fine in a screenshot, `sshd` rejected the key with
  no useful client-side error) — verify with `diff` against a known-good
  copy before trusting it, or just use the ISO method instead.

Once the key's in place:
```bash
ssh omarchy-kids-child "whoami"   # should succeed with no password prompt
```

## 8. Root SSH access (dev convenience, not production)

Saves constant `sudo` password prompts while iterating. This has nothing
to do with the production agent design (see `Omarchy Kids - Implementierung
Agent` in the vault) — it's purely for driving a throwaway dev VM.

In the VM (you'll type the sudo password yourself, live):
```bash
sudo mkdir -p /root/.ssh && sudo cp ~/.ssh/authorized_keys /root/.ssh/authorized_keys \
  && sudo chown root:root /root/.ssh/authorized_keys && sudo chmod 700 /root/.ssh \
  && sudo chmod 600 /root/.ssh/authorized_keys
```

Add a second `~/.ssh/config` entry (`omarchy-kids-child-root`, `User root`,
same `IdentityFile`).

Do **not** try to additionally set up passwordless (`NOPASSWD`) sudo for
the regular user as a way around this — that's a meaningfully bigger
privilege change than a scoped SSH key, and got blocked by this
environment's own safety classifier when tried. Root-over-SSH is the
better-scoped option anyway: it's one clearly-labeled, easily-revoked
credential instead of a standing sudoers change.

## 9. Clipboard between host and VM (optional, nice to have)

```bash
ssh omarchy-kids-child-root "pacman -S --needed --noconfirm spice-vdagent"
```

Needs the firewall fix from §2 (internet access) already in place. Once
installed, `Ctrl+Shift+V` works normally between host and the
`virt-manager` console window.

## Fast iteration: snapshots + a scripted pairing round trip

Sections 3-7 above (fresh ISO install through first SSH access) take real
wall-clock time and, worse, a chunk of it (§5) is interactive — fine once,
tedious every time you want to re-test the setup wizard or a pairing change.
Two scripts in `scripts/` cut that down for the parts that don't need a
truly fresh install:

- **`scripts/vm-snapshot.sh save|restore|list [name]`** — wraps `virsh
  snapshot-create-as`/`snapshot-revert`, an internal qcow2 snapshot of the
  whole VM (disk + NVRAM). Take one right after §7 (`save fresh-boot`, VM
  shut off) — account created, network/SSH working, nothing else touched —
  then `restore fresh-boot` before each test pass instead of reinstalling.
  Needs the qcow2 NVRAM format from §4; see "Known gotchas" below if this
  errors on an older VM.
- **`scripts/vm-pairing-smoke-test.sh <host> <user>`** — automates "playing
  through the pairing" against a VM that already has `omarchy-kids-agent`/
  `omarchy-kids-pairing` on PATH (i.e. past `omarchy-kids-bootstrap`, see
  the "Quick reference" section below): starts `omarchy-kids-pairing serve`
  on the child over SSH, dials in with the reference `pair --yes` CLI
  instead of the real Control Center GUI, and checks the machine-readable
  `PAIR_RESULT:` line plus a live `agent status` call through the freshly
  installed key — no `virsh screenshot` eyeballing needed.

This does **not** replace the full from-ISO test in `docs/agent-protocol.md`'s
"End-to-end verification" section (driven via `virsh screenshot`/`send-key`
through the real tty1 `gum` form) — blind keystroke automation of an
interactive console form is exactly the kind of thing that's fragile (see
the idle-lock gotcha below), so that full chain is still worth re-running by
hand periodically, especially after touching `setup-wizard/first-boot/`.
The two scripts above are a fast, scriptable stand-in for the *pairing
protocol* specifically, for iterating on `agent/pairing/` or
`control/gui/PairingDialog` without redoing the account/tier setup every
time.

## Known gotchas, collected

- **UFW blocks things at three independent layers**: host-INPUT (DHCP/SSH
  *to* the VM), host-FORWARD (VM's *outbound* internet), and the guest's
  own UFW (SSH *into* the VM). Hitting one doesn't mean you've hit all of
  them — check `ufw status` on both sides when something inexplicably times
  out.
- **`sudo` over a background/non-interactive SSH or Bash session can fail**
  with `a terminal is required to read the password` even when the same
  command works fine interactively, if your `sudo` needs any kind of
  interactive auth (password, biometric, ...) — that needs a live session.
  Prefer running things in the foreground, or use the root SSH key from §8
  to sidestep sudo entirely.
- **Quickshell third-party overlay/menu plugins are opt-in.** Dropping a
  plugin into `~/.config/omarchy/plugins/<id>/` is not enough — it also
  needs `{"id": "<id>"}` in `~/.config/omarchy/shell.json`'s `plugins[]`
  array, or it silently no-ops (`journalctl _PID=<quickshell-pid>` shows
  `plugin not enabled, not summoning` — the only trace). `omarchy-kids-set-tier`
  handles this automatically for tiers that ship a `launcher/`.
- **Structural QML plugin changes need a full shell restart**, not just a
  file save — Quickshell's live-reload picks up data/minor changes but held
  onto a stale, broken layout through several clean close/reopen cycles
  until the process itself was restarted: `pkill -f 'quickshell -n -p'`
  (it respawns automatically via Hyprland's autostart). `omarchy-kids-set-tier`
  does this automatically too.
- **`omarchy-theme-set` (and anything else that should visibly affect the
  running desktop) run over SSH doesn't visibly apply** — it updates the
  on-disk state fine, but the command runs outside the graphical session
  (no `WAYLAND_DISPLAY`/`DBUS_SESSION_BUS_ADDRESS`/`HYPRLAND_INSTANCE_SIGNATURE`),
  so the running compositor never gets the reload signal. Only matters for
  *driving* the VM over SSH during dev — see the `Omarchy Kids -
  Implementierung Agent` vault note for why this also shapes the
  agent/agentd split in the real design.
- **VM screen locks on idle** and blind `virsh send-key` automation can't
  unlock it (no known password to type). Either disable the idle
  lock/timeout for the dev VM, or keep unlocking it yourself when it
  matters mid-session.
- **`virsh snapshot-create-as` refuses a VM with the default raw NVRAM
  format**: "internal snapshots of a VM with pflash based firmware require
  QCOW2 nvram format" (check yours with `virsh dumpxml <domain> | grep
  nvram`; stock `virt-install --boot uefi` produces `format='raw'`). §4
  now creates new VMs with qcow2 NVRAM from the start. Root cause, if
  you're debugging this yourself: as of libvirt ~10.10, the NVRAM store's
  `format` must exactly match its `templateFormat` — libvirt refuses to
  convert between them at start time, even if the store file already
  exists on disk in the "wrong" format ("`Operation not supported:
  conversion of the nvram template to another target format is not
  supported`"). Since Arch's `edk2-ovmf` only ships a raw template, the
  simple `--boot uefi,nvram.templateFormat=qcow2` form (which asks libvirt
  to *auto-select* a firmware descriptor matching that templateFormat)
  always fails with "`Unable to find 'efi' firmware that is compatible
  with the current configuration`" — no installed descriptor declares a
  qcow2 template. §4's explicit `loader=`/`nvram.template=<our own
  pre-converted qcow2 template>` form sidesteps the auto-selection
  matching entirely. Also: `virt-xml`/`virt-install`'s `--boot` CLI has no
  `nvram.format` property at all (checked `virtinst/domain/os.py` —only
  `nvram`, `nvram.template`, `nvram.templateFormat` are mapped), so the
  `format='qcow2'` attribute can only be set by hand-editing the domain
  XML — there's no CLI incantation for it.

  **Verified migration for an existing raw-NVRAM VM** (2026-08-30, full
  round trip incl. snapshot + revert against the real dev VM — domain shut
  off first):
  ```bash
  sudo qemu-img convert -O qcow2 \
    /var/lib/libvirt/qemu/nvram/<domain>_VARS.fd \
    /var/lib/libvirt/qemu/nvram/<domain>_VARS.qcow2
  sudo cp /var/lib/libvirt/qemu/nvram/<domain>_VARS.qcow2 \
          /var/lib/libvirt/qemu/nvram/<domain>_VARS.qcow2.template
  sudo chmod 0644 /var/lib/libvirt/qemu/nvram/<domain>_VARS.qcow2{,.template}

  virsh dumpxml <domain> > /tmp/<domain>.xml
  ```
  Then hand-edit `/tmp/<domain>.xml`:
  - Remove any `firmware='efi'` attribute on the `<os>` tag, and any
    separate `<firmware>...</firmware>` block — both re-trigger the same
    firmware auto-selection matching that fails above, even alongside an
    otherwise-explicit `<loader>`/`<nvram>`.
  - Replace the `<nvram>` line with:
    ```xml
    <nvram template='/var/lib/libvirt/qemu/nvram/<domain>_VARS.qcow2.template' templateFormat='qcow2' format='qcow2'>/var/lib/libvirt/qemu/nvram/<domain>_VARS.qcow2</nvram>
    ```
  ```bash
  virsh define /tmp/<domain>.xml
  virsh start <domain>
  ```
  The original `_VARS.fd` is left in place untouched as a fallback. Note
  `scripts/vm-snapshot.sh` itself needed a fix alongside this (see its
  header comment) — it called `sudo virsh`, which fails silently in a
  non-interactive context and made an early test of this look like the
  revert had done nothing.
- **Scripting `sudo` over SSH needs `ssh -tt` + `sudo -S`, not plain
  `sudo`.** A bare `ssh host "sudo cmd"` fails with "a terminal is required
  to read the password" — `sudo` insists on opening `/dev/tty` directly,
  ignoring stdin, unless told otherwise. `sudo -S` makes it read the
  password from stdin instead (`echo "$PASSWORD" | sudo -S cmd`); `ssh -tt`
  forces a pseudo-tty so the remote shell behaves like an interactive
  session. Combine both, and the fixed dev password (see "Fast path"
  above) makes every `sudo`-requiring step in `vm-deploy-agent.sh`/
  `vm-pair-for-dashboard.sh` fully non-interactive. No `sshpass`/`expect`
  needed. Also worth knowing: `virsh`/`virt-install` themselves never need
  `sudo` at all on a host where your user is in the `libvirt` group (which
  this doc's §1 already sets up) — plain `virsh -c qemu:///system ...`
  (not `sudo virsh`, and not bare `virsh`, which defaults to the
  unprivileged `qemu:///session` and silently doesn't see your domains) is
  enough, including for `vol-upload`/`vol-create-as` into a storage pool —
  a good way to get a file onto the host's libvirt-managed storage without
  ever needing your own `sudo` password (used by `vm-recreate.sh` to avoid
  needing a human for the key-ISO step).

## Quick reference: iterating on `tiers/`

```bash
rsync -av --delete tiers/ omarchy-kids-child:~/omarchy-kids-tiers/
ssh omarchy-kids-child "chmod +x ~/omarchy-kids-tiers/omarchy-kids-set-tier && ~/omarchy-kids-tiers/omarchy-kids-set-tier mini"

# Visual check without touching the VM's own input:
virsh -c qemu:///system screenshot omarchy-kids-child /tmp/check.png

# Fast pairing iteration (see "Fast iteration" above) — revert to a
# pre-pairing snapshot, then run the round trip against it:
./scripts/vm-snapshot.sh restore fresh-boot
./scripts/vm-pairing-smoke-test.sh 192.168.122.109 <child_user>

# Full from-scratch recreation (see "Fast path" at the top) instead of
# reinstalling by hand:
./scripts/vm-recreate.sh
./scripts/vm-deploy-agent.sh <ip>
./scripts/vm-pair-for-dashboard.sh <ip> Testkind
./scripts/vm-snapshot.sh save fresh-boot

# Turn that bare-but-paired VM into an actual kids computer for feature
# testing (see "Two snapshots, two test scenarios" above):
./scripts/vm-apply-tier.sh <ip> mini
./scripts/vm-snapshot.sh save kiosk-ready
```
