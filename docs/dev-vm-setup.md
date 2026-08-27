# Dev VM setup: fresh Omarchy child computer

Step-by-step for standing up a throwaway Omarchy VM on your own machine to
develop/test `tiers/`, the launcher plugin, and (later) the agent/control
center against. Written from doing this once on a Linux laptop running
Omarchy natively as the host; nothing below is specific to that hardware.

Everything here is dev-only. None of it — the root SSH access, the ISO
handling, the blind-keyboard-typing fallback — is part of the production
setup-wizard/pairing flow that ships to an actual child computer.

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
  --boot uefi \
  --features smm=off \
  --noautoconsole \
  --wait -1
```

Adjust `--memory`/`--vcpus` to the host's headroom (`free -h`) — 6GB/4 vCPUs
assumes an 8-16GB host that's also running other things. UEFI without
secure boot (`--boot uefi`, no secboot vars) and `smm=off` matches Omarchy's
own requirement to have Secure Boot/TPM disabled.

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

## Quick reference: iterating on `tiers/`

```bash
rsync -av --delete tiers/ omarchy-kids-child:~/omarchy-kids-tiers/
ssh omarchy-kids-child "chmod +x ~/omarchy-kids-tiers/omarchy-kids-set-tier && ~/omarchy-kids-tiers/omarchy-kids-set-tier 5-7"

# Visual check without touching the VM's own input:
sudo virsh screenshot omarchy-kids-child /tmp/check.png
```
