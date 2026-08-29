#!/bin/bash
# Save/restore the dev VM's disk state via libvirt's own internal qcow2
# snapshots, so iterating on the setup wizard/pairing doesn't need a full
# reinstall from the ISO (docs/dev-vm-setup.md §3-5) on every run — take one
# snapshot right after first boot (account created, network/SSH up, nothing
# else touched yet) and revert to it before each test pass.
#
# Requires the domain's NVRAM to be in qcow2 format: `virsh snapshot-create-as`
# snapshots NVRAM together with the disk, and refuses on the default raw
# format ("internal snapshots of a VM with pflash based firmware require
# QCOW2 nvram format" — hit this against the real dev VM, 2026-08-30, see
# docs/dev-vm-setup.md "Known gotchas"). VMs created with the `--boot
# uefi,nvram.templateFormat=qcow2` flag in dev-vm-setup.md §4 already have
# this; an existing VM created before that needs the one-time migration
# documented there first.
#
# Usage:
#   vm-snapshot.sh save <name>       # domain must be shut off
#   vm-snapshot.sh restore <name>    # domain must be shut off
#   vm-snapshot.sh list
set -euo pipefail

DOMAIN="${OMARCHY_KIDS_DEV_VM:-omarchy-kids-child}"

usage() {
  echo "Usage: vm-snapshot.sh <save|restore|list> [name]" >&2
  exit 1
}

cmd="${1:-}"
case "$cmd" in
  save)
    name="${2:?Usage: vm-snapshot.sh save <name>}"
    sudo virsh snapshot-create-as "$DOMAIN" "$name" "dev snapshot: $name ($(date --iso-8601=seconds))"
    ;;
  restore)
    name="${2:?Usage: vm-snapshot.sh restore <name>}"
    sudo virsh snapshot-revert "$DOMAIN" "$name"
    ;;
  list)
    sudo virsh snapshot-list "$DOMAIN"
    ;;
  *)
    usage
    ;;
esac
