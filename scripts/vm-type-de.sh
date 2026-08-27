#!/bin/bash
# Types text into a running libvirt VM's console via `virsh send-key`,
# character by character — for bootstrapping a VM before SSH access exists
# (e.g. typing an SSH pubkey into ~/.ssh/authorized_keys by hand).
#
# Assumes the GUEST is using a German (QWERTZ) keyboard layout, which is
# what a German locale/keyboard choice during Omarchy install produces.
# `virsh send-key` sends raw scancodes; the guest's active layout decides
# what character comes out, so this mapping is layout-specific — it will
# type the wrong characters against a US-layout guest. Swap the case
# statement below (or find/adjust one) if you're driving a US-layout VM.
#
# Usage: vm-type-de.sh <domain> "text to type"
#
# Known limits: no support for characters outside what's mapped below
# (unmapped characters print a warning and are skipped, not silently
# dropped mid-command — check stderr). Sending many keys fast can drop
# keystrokes under load; this waits 60ms between each as a compromise
# between speed and reliability. For anything beyond a short command,
# prefer the ISO-mount trick in docs/dev-vm-setup.md instead of typing —
# it's immune to both layout and drop issues.
set -euo pipefail

dom="${1:?Usage: vm-type-de.sh <domain> <text>}"
text="${2:?Usage: vm-type-de.sh <domain> <text>}"

send() {
  virsh send-key "$dom" --codeset linux "$@" >/dev/null 2>&1
}

for (( i=0; i<${#text}; i++ )); do
  c="${text:$i:1}"
  case "$c" in
    y) send KEY_Z ;;
    z) send KEY_Y ;;
    Y) send KEY_LEFTSHIFT KEY_Z ;;
    Z) send KEY_LEFTSHIFT KEY_Y ;;
    [a-x]) send "KEY_${c^^}" ;;
    [A-X]) send KEY_LEFTSHIFT "KEY_${c}" ;;
    [0-9]) send "KEY_${c}" ;;
    " ") send KEY_SPACE ;;
    "-") send KEY_SLASH ;;
    "_") send KEY_LEFTSHIFT KEY_SLASH ;;
    "/") send KEY_LEFTSHIFT KEY_7 ;;
    "+") send KEY_RIGHTBRACE ;;
    "*") send KEY_LEFTSHIFT KEY_RIGHTBRACE ;;
    ".") send KEY_DOT ;;
    ":") send KEY_LEFTSHIFT KEY_DOT ;;
    ",") send KEY_COMMA ;;
    ";") send KEY_LEFTSHIFT KEY_COMMA ;;
    "=") send KEY_LEFTSHIFT KEY_0 ;;
    "~") send KEY_RIGHTALT KEY_RIGHTBRACE ;;
    "@") send KEY_RIGHTALT KEY_Q ;;
    "\\") send KEY_RIGHTALT KEY_MINUS ;;
    '"') send KEY_LEFTSHIFT KEY_2 ;;
    "!") send KEY_LEFTSHIFT KEY_1 ;;
    "&") send KEY_LEFTSHIFT KEY_6 ;;
    "(") send KEY_LEFTSHIFT KEY_8 ;;
    ")") send KEY_LEFTSHIFT KEY_9 ;;
    "\$") send KEY_LEFTSHIFT KEY_4 ;;
    "%") send KEY_LEFTSHIFT KEY_5 ;;
    "#") send KEY_BACKSLASH ;;
    ">") send KEY_LEFTSHIFT KEY_102ND ;;
    "<") send KEY_102ND ;;
    "|") send KEY_RIGHTALT KEY_102ND ;;
    "'") send KEY_RIGHTALT KEY_BACKSLASH ;;
    *) echo "vm-type-de.sh: unmapped character '$c', skipped" >&2 ;;
  esac
  sleep 0.06
done
