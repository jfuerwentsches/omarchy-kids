#!/bin/bash
# Fully automated dev-VM recreation using Omarchy's own unattended-install
# mechanism (https://omarchy.org/manual/unattended-installs/) rather than
# blind console keystroke automation. Destroys/undefines the existing
# domain (if any), builds a "cidata" autoinstall volume (see
# vm-cidata-build.sh), and boots a fresh VM with both the Omarchy ISO and
# the cidata volume attached — the installer detects cidata, skips its own
# interactive wizard entirely, and (because the cidata volume carries an
# authorized_keys file) enables sshd and opens ufw for SSH on its own. The
# only thing this script actually waits on is the install finishing, the
# reboot, and SSH coming up — no keystrokes sent at all.
#
# Why not console automation: two earlier attempts (2026-08-30) used
# `virsh send-key` timed against fixed sleeps, then against a screenshot-
# stability check — both got desynced from the actual screen under real
# host load (background VMs/work), corrupting the account-setup sequence
# in ways that were only discovered much later, deep in the run. The
# cidata approach removes the interactive wizard from the picture, so
# there's nothing left to desync from.
#
# This intentionally replaces essentially all of docs/dev-vm-setup.md
# §3-6 for the common case. Still separate: `scripts/vm-deploy-agent.sh`
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
DEV_KEY="$HOME/.ssh/omarchy_kids_dev"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VIRSH="virsh -c qemu:///system"

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

if [ ! -f "$DEV_KEY" ]; then
  ssh-keygen -t ed25519 -f "$DEV_KEY" -N "" -C "omarchy-kids-dev"
fi

echo "==> Building the cidata autoinstall volume"
"$SCRIPT_DIR/vm-cidata-build.sh" "$CHILD_USER" "$CHILD_PASSWORD" "$CHILD_HOSTNAME" "$DEV_KEY.pub" 40

echo "==> Creating '$DOMAIN'"
virt-install \
  --connect qemu:///system \
  --name "$DOMAIN" \
  --memory 4096 \
  --vcpus 2 \
  --cpu host-passthrough \
  --disk pool=default,size=40,format=qcow2,bus=virtio \
  --cdrom "$ISO" \
  --disk /var/lib/libvirt/boot/cidata.iso,device=cdrom,bus=sata \
  --os-variant archlinux \
  --network network=default,model=virtio \
  --graphics spice \
  --video virtio \
  --boot "loader=/usr/share/edk2/x64/OVMF_CODE.4m.fd,loader.readonly=yes,loader.type=pflash,nvram.template=$NVRAM_TEMPLATE,nvram.templateFormat=qcow2" \
  --features smm=off \
  --noautoconsole \
  --wait 0

echo "==> Installing unattended — no keystrokes needed. Polling every 15s"
echo "    (typically ~5 minutes; inspect with: virsh -c qemu:///system screenshot $DOMAIN /tmp/check.png)"
for _ in $(seq 1 60); do
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

echo "==> Waiting for a DHCP lease"
ip=""
for _ in $(seq 1 60); do
  ip=$($VIRSH net-dhcp-leases default 2>/dev/null | grep -F "$DOMAIN" | grep -oE '192\.168\.122\.[0-9]+' | tail -1)
  [ -n "$ip" ] && break
  sleep 5
done
if [ -z "$ip" ]; then
  echo "!! Could not find a DHCP lease for '$DOMAIN' — check 'virsh net-dhcp-leases default' by hand." >&2
  exit 1
fi

echo "==> Waiting for SSH ($CHILD_USER@$ip) — sshd/ufw were configured by the installer itself"
ok=0
for _ in $(seq 1 30); do
  if timeout 5 ssh -o BatchMode=yes -o ConnectTimeout=3 -o StrictHostKeyChecking=accept-new -i "$DEV_KEY" "$CHILD_USER@$ip" true 2>/dev/null; then
    ok=1
    break
  fi
  sleep 5
done
if [ "$ok" != 1 ]; then
  echo "!! SSH didn't come up — inspect with: virsh -c qemu:///system screenshot $DOMAIN /tmp/check.png" >&2
  exit 1
fi

echo "==> Done. $DOMAIN is up at $ip."
echo "    Update ~/.ssh/config's HostName for omarchy-kids-child to $ip."
echo "    Next: scripts/vm-deploy-agent.sh, then pair, then vm-snapshot.sh save fresh-boot."
