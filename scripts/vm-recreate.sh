#!/bin/bash
# Fully automated dev-VM recreation: destroys/undefines the existing
# domain (if any) and drives a brand-new one all the way from `virt-install`
# through the interactive Omarchy installer/first-login and basic network/
# SSH bring-up, using FIXED, PUBLIC, DEV-ONLY credentials (see below) so no
# human needs to type anything at the console. Verified end-to-end
# 2026-08-30 against a real run — see docs/dev-vm-setup.md for the full
# background and root causes behind each step.
#
# This intentionally replaces the interactive parts of docs/dev-vm-setup.md
# §3-6 for the common case. Still manual/separate: `scripts/vm-deploy-agent.sh`
# (agent binaries + pairing) and `scripts/vm-snapshot.sh` (fast reset
# afterward) — this script's job ends at "SSH as $CHILD_USER works".
#
# Usage: vm-recreate.sh
#   Env overrides: OMARCHY_KIDS_DEV_VM (domain name), OMARCHY_ISO (path)
set -euo pipefail

DOMAIN="${OMARCHY_KIDS_DEV_VM:-omarchy-kids-child}"
ISO="${OMARCHY_ISO:-/var/lib/libvirt/boot/omarchy-4.0.1.iso}"
NVRAM_TEMPLATE=/var/lib/libvirt/boot/OVMF_VARS.4m.qcow2

# Fixed dev-only account. NOT a secret worth protecting — this VM lives on
# an isolated libvirt NAT network (192.168.122.0/24), is never reachable
# from outside the host, and is treated as throwaway by design (see
# docs/dev-vm-setup.md's opening paragraph). Documented in plain text here
# on purpose, the same way Vagrant boxes ship a well-known default keypair.
CHILD_USER="devchild"
CHILD_PASSWORD="omarchy-kids-dev"
CHILD_HOSTNAME="omarchy-kids-child"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TYPE_US="$SCRIPT_DIR/vm-type-us.sh"
VIRSH="virsh -c qemu:///system"

send() { $VIRSH send-key "$DOMAIN" --codeset linux "$@"; }
type_us() { "$TYPE_US" "$DOMAIN" "$1"; }
enter() { send KEY_ENTER; }

echo "==> Destroying/undefining any existing '$DOMAIN' domain"
$VIRSH destroy "$DOMAIN" >/dev/null 2>&1 || true
for snap in $($VIRSH snapshot-list "$DOMAIN" --name 2>/dev/null || true); do
  $VIRSH snapshot-delete "$DOMAIN" "$snap" >/dev/null 2>&1 || true
done
$VIRSH undefine "$DOMAIN" --nvram >/dev/null 2>&1 || true
$VIRSH vol-delete --pool default "$DOMAIN.qcow2" >/dev/null 2>&1 || true

if ! $VIRSH vol-list boot 2>/dev/null | grep -q OVMF_VARS.4m.qcow2; then
  echo "==> One-time: converting the raw OVMF NVRAM template to qcow2"
  tmp=$(mktemp -d)
  qemu-img convert -O qcow2 /usr/share/edk2/x64/OVMF_VARS.4m.fd "$tmp/OVMF_VARS.4m.qcow2"
  size=$(stat -c%s "$tmp/OVMF_VARS.4m.qcow2")
  $VIRSH vol-create-as boot OVMF_VARS.4m.qcow2 "$size"
  $VIRSH vol-upload --pool boot OVMF_VARS.4m.qcow2 "$tmp/OVMF_VARS.4m.qcow2"
  rm -rf "$tmp"
fi

echo "==> Creating '$DOMAIN'"
virt-install \
  --connect qemu:///system \
  --name "$DOMAIN" \
  --memory 4096 \
  --vcpus 2 \
  --cpu host-passthrough \
  --disk pool=default,size=40,format=qcow2,bus=virtio \
  --cdrom "$ISO" \
  --os-variant archlinux \
  --network network=default,model=virtio \
  --graphics spice \
  --video virtio \
  --boot "loader=/usr/share/edk2/x64/OVMF_CODE.4m.fd,loader.readonly=yes,loader.type=pflash,nvram.template=$NVRAM_TEMPLATE,nvram.templateFormat=qcow2" \
  --features smm=off \
  --noautoconsole \
  --wait 0

echo "==> Waiting for the Omarchy installer welcome screen"
sleep 20
enter  # "Press Return to Start Install"
sleep 3

echo "==> Keyboard layout: accepting default (English (US))"
enter
sleep 2

echo "==> Username"
type_us "$CHILD_USER"; enter; sleep 2
echo "==> Password (+ confirm)"
type_us "$CHILD_PASSWORD"; enter; sleep 2
type_us "$CHILD_PASSWORD"; enter; sleep 2
echo "==> Full name / email: skipping"
enter; sleep 2
enter; sleep 2
echo "==> Hostname"
type_us "$CHILD_HOSTNAME"; enter; sleep 2
echo "==> Timezone: accepting auto-detected default"
enter; sleep 2
echo "==> Confirming account summary"
enter; sleep 3
echo "==> Disk selection: accepting the only disk"
enter; sleep 2
echo "==> Disabling disk encryption (Ctrl+C), confirming install"
send KEY_LEFTCTRL KEY_C; sleep 1
enter

echo "==> Installing — this takes a few minutes, polling every 15s"
for _ in $(seq 1 40); do
  sleep 15
  state=$($VIRSH domstate "$DOMAIN" 2>/dev/null || echo unknown)
  if [ "$state" != "running" ]; then
    echo "    domain state changed to '$state' — install likely finished and rebooted"
    break
  fi
done

echo "==> Restarting after install if needed"
if [ "$($VIRSH domstate "$DOMAIN" 2>/dev/null)" != "running" ]; then
  $VIRSH start "$DOMAIN"
fi

echo "==> Waiting for first graphical login screen"
sleep 25
echo "==> Logging in"
type_us "$CHILD_PASSWORD"; enter
sleep 8

echo "==> Opening a terminal"
send KEY_LEFTMETA KEY_ENTER
sleep 2

echo "==> Enabling sshd + ufw (password typed automatically)"
type_us "sudo systemctl enable --now sshd"; enter; sleep 2
type_us "$CHILD_PASSWORD"; enter; sleep 2
type_us "sudo ufw allow ssh"; enter; sleep 2

echo "==> Building a one-off SSH key ISO and mounting it in the guest"
DEV_KEY="$HOME/.ssh/omarchy_kids_dev"
if [ ! -f "$DEV_KEY" ]; then
  ssh-keygen -t ed25519 -f "$DEV_KEY" -N "" -C "omarchy-kids-dev"
fi
tmp=$(mktemp -d)
mkdir -p "$tmp/keydata"
cp "$DEV_KEY.pub" "$tmp/keydata/authorized_keys"
genisoimage -output "$tmp/key.iso" -volid KEYDATA -joliet -rock "$tmp/keydata/" >/dev/null 2>&1
$VIRSH vol-delete --pool boot key.iso >/dev/null 2>&1 || true
size=$(stat -c%s "$tmp/key.iso")
$VIRSH vol-create-as boot key.iso "$size"
$VIRSH vol-upload --pool boot key.iso "$tmp/key.iso"
rm -rf "$tmp"
$VIRSH change-media "$DOMAIN" sda --source /var/lib/libvirt/boot/key.iso --insert --live

type_us "udisksctl mount -b /dev/sr0"; enter; sleep 2
type_us "rm -f ~/.ssh && mkdir -p ~/.ssh && cp /run/media/$CHILD_USER/KEYDATA/authorized_keys ~/.ssh/ && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys"
enter; sleep 2
$VIRSH change-media "$DOMAIN" sda --eject --live >/dev/null 2>&1 || true

echo "==> Waiting for a DHCP lease"
ip=""
for _ in $(seq 1 20); do
  ip=$($VIRSH net-dhcp-leases default 2>/dev/null | grep -F "$DOMAIN" | grep -oE '192\.168\.122\.[0-9]+' | tail -1)
  [ -n "$ip" ] && break
  sleep 3
done
if [ -z "$ip" ]; then
  echo "!! Could not find a DHCP lease for '$DOMAIN' — check 'virsh net-dhcp-leases default' by hand." >&2
  exit 1
fi

echo "==> Verifying SSH ($CHILD_USER@$ip)"
if ! timeout 10 ssh -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new -i "$DEV_KEY" "$CHILD_USER@$ip" true; then
  echo "!! SSH didn't come up — inspect with: virsh -c qemu:///system screenshot $DOMAIN /tmp/check.png" >&2
  exit 1
fi

echo "==> Done. $DOMAIN is up at $ip."
echo "    Update ~/.ssh/config's HostName for omarchy-kids-child to $ip."
echo "    Next: scripts/vm-deploy-agent.sh, then pair, then vm-snapshot.sh save fresh-boot."
