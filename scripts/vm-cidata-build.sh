#!/bin/bash
# Builds an Omarchy "unattended install" cidata volume and uploads it into
# libvirt's `boot` storage pool as cidata.iso, ready to attach as a second
# cdrom to virt-install. See https://omarchy.org/manual/unattended-installs/
# and, for the exact JSON the installer expects, the real source: the
# `configurator` script and `orchestrator/context.py` in
# github.com/omacom-io/omarchy-iso (write_user_files()/the user_configuration.json
# heredoc). This intentionally builds the same JSON that script would, for a
# fixed 40G /dev/vda disk, unencrypted, en_US keyboard, no LUKS —
# reproducing the configurator's own partition-size arithmetic rather than
# needing to capture a live copy off a real install.
#
# Replaces the account/disk/network portion of vm-recreate.sh's console
# automation entirely: an authorized_keys file on the cidata drive makes the
# installer enable sshd and open ufw for SSH on its own (see
# phases_impl.py's configure_ssh_access) — found 2026-08-30 after two
# console-automation runs corrupted the account-setup screen under host
# load (fixed timing can't be trusted; see vm-recreate.sh's `settle()`
# comment for the first fix attempt and why it wasn't enough on its own).
# With a cidata drive attached, the installer needs no keystrokes at all
# for account/disk/keyboard/network — only the interactive "Press Return to
# Start Install" welcome screen remains.
#
# Usage: vm-cidata-build.sh <username> <password> <hostname> <ssh-pubkey-file> [disk-size-gib]
set -euo pipefail

USERNAME="${1:?Usage: vm-cidata-build.sh <username> <password> <hostname> <ssh-pubkey-file> [disk-size-gib]}"
PASSWORD="${2:?}"
HOSTNAME_="${3:?}"
PUBKEY_FILE="${4:?}"
DISK_GIB="${5:-40}"

VIRSH="virsh -c qemu:///system"
PASSWORD_HASH=$(openssl passwd -6 "$PASSWORD")

MIB=$((1024 * 1024))
GIB=$((MIB * 1024))
DISK_SIZE=$((DISK_GIB * GIB))
DISK_SIZE_IN_MIB=$(( (DISK_SIZE / MIB) * MIB ))
GPT_BACKUP_RESERVE=$MIB
BOOT_PARTITION_START=$MIB
BOOT_PARTITION_SIZE=$((2 * GIB))
MAIN_PARTITION_START=$((BOOT_PARTITION_SIZE + BOOT_PARTITION_START))
MAIN_PARTITION_SIZE=$((DISK_SIZE_IN_MIB - MAIN_PARTITION_START - GPT_BACKUP_RESERVE))

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/cidata"

cat >"$tmp/cidata/user_configuration.json" <<EOF
{
    "app_config": null,
    "archinstall-language": "English",
    "auth_config": {},
    "audio_config": { "audio": "pipewire" },
    "bootloader_config": { "bootloader": "Limine", "uki": false, "removable": false },
    "custom_commands": [],
    "omarchy_install": {
        "mode": "full_disk",
        "defer_provisioning": false,
        "target_mount": "/mnt",
        "boot": {
            "esp_mount": "/boot",
            "esp_path": "/EFI/limine",
            "efi_binary": "limine_x64.efi",
            "enable_fallback": true
        },
        "storage": { "kernel": "linux" }
    },
    "disk_config": {
        "config_type": "default_layout",
        "device_modifications": [
            {
                "device": "/dev/vda",
                "partitions": [
                    {
                        "btrfs": [],
                        "dev_path": null,
                        "flags": [ "boot", "esp" ],
                        "fs_type": "fat32",
                        "mount_options": [],
                        "mountpoint": "/boot",
                        "obj_id": "ea21d3f2-82bb-49cc-ab5d-6f81ae94e18d",
                        "size": {
                            "sector_size": { "unit": "B", "value": 512 },
                            "unit": "B",
                            "value": $BOOT_PARTITION_SIZE
                        },
                        "start": {
                            "sector_size": { "unit": "B", "value": 512 },
                            "unit": "B",
                            "value": $BOOT_PARTITION_START
                        },
                        "status": "create",
                        "type": "primary"
                    },
                    {
                        "btrfs": [
                            { "mountpoint": "/", "name": "@" },
                            { "mountpoint": "/home", "name": "@home" },
                            { "mountpoint": "/var/log", "name": "@log" },
                            { "mountpoint": "/var/cache/pacman/pkg", "name": "@pkg" }
                        ],
                        "dev_path": null,
                        "flags": [],
                        "fs_type": "btrfs",
                        "mount_options": [ "compress=zstd" ],
                        "mountpoint": null,
                        "obj_id": "8c2c2b92-1070-455d-b76a-56263bab24aa",
                        "size": {
                            "sector_size": { "unit": "B", "value": 512 },
                            "unit": "B",
                            "value": $MAIN_PARTITION_SIZE
                        },
                        "start": {
                            "sector_size": { "unit": "B", "value": 512 },
                            "unit": "B",
                            "value": $MAIN_PARTITION_START
                        },
                        "status": "create",
                        "type": "primary"
                    }
                ],
                "wipe": true
            }
        ]
    },
    "hostname": "$HOSTNAME_",
    "kernels": [ "linux" ],
    "network_config": { "type": "iso" },
    "ntp": true,
    "parallel_downloads": 8,
    "script": null,
    "services": [],
    "swap": true,
    "timezone": "Europe/Berlin",
    "locale_config": { "kb_layout": "us", "sys_enc": "UTF-8", "sys_lang": "en_US.UTF-8" },
    "mirror_config": {
        "custom_repositories": [],
        "custom_servers": [
            {"url": "https://mirror.omarchy.org/\$repo/os/\$arch"},
            {"url": "https://mirror.rackspace.com/archlinux/\$repo/os/\$arch"},
            {"url": "https://geo.mirror.pkgbuild.com/\$repo/os/\$arch"}
        ],
        "mirror_regions": {},
        "optional_repositories": []
    },
    "packages": [
        "base-devel",
        "git",
        "omarchy-keyring",
        "omarchy-settings",
        "omarchy"
    ],
    "profile_config": {
        "gfx_driver": null,
        "greeter": null,
        "profile": {}
    },
    "version": "3.0.9"
}
EOF

cat >"$tmp/cidata/user_credentials.json" <<EOF
{
    "root_enc_password": $(printf '%s' "$PASSWORD_HASH" | jq -Rsa),
    "users": [
        {
            "enc_password": $(printf '%s' "$PASSWORD_HASH" | jq -Rsa),
            "groups": [],
            "sudo": true,
            "username": $(printf '%s' "$USERNAME" | jq -Rsa)
        }
    ]
}
EOF

cp "$PUBKEY_FILE" "$tmp/cidata/authorized_keys"

genisoimage -output "$tmp/cidata.iso" -volid cidata -joliet -rock "$tmp/cidata/" >/dev/null 2>&1

$VIRSH vol-delete --pool boot cidata.iso >/dev/null 2>&1 || true
size=$(stat -c%s "$tmp/cidata.iso")
$VIRSH vol-create-as boot cidata.iso "$size"
$VIRSH vol-upload --pool boot cidata.iso "$tmp/cidata.iso"

echo "cidata.iso uploaded to the 'boot' pool (/var/lib/libvirt/boot/cidata.iso)."
