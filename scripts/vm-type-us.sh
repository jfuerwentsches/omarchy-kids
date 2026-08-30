#!/bin/bash
# Types text into a running libvirt VM's console via `virsh send-key`,
# character by character — companion to vm-type-de.sh for a US (QWERTY)
# guest keyboard layout. Mapping is much simpler than the German one since
# `virsh send-key`'s KEY_* identifiers already follow the US layout for
# unshifted letters/digits and most punctuation.
#
# Usage: vm-type-us.sh <domain> "text to type"
#
# Same known limits as vm-type-de.sh: unmapped characters print a warning
# and are skipped (check stderr); 60ms between keys as a speed/reliability
# compromise; prefer the ISO-mount trick in docs/dev-vm-setup.md for
# anything beyond a short command.
set -euo pipefail

dom="${1:?Usage: vm-type-us.sh <domain> <text>}"
text="${2:?Usage: vm-type-us.sh <domain> <text>}"

send() {
  virsh -c qemu:///system send-key "$dom" --codeset linux "$@" >/dev/null 2>&1
}

for (( i=0; i<${#text}; i++ )); do
  c="${text:$i:1}"
  case "$c" in
    [a-z]) send "KEY_${c^^}" ;;
    [A-Z]) send KEY_LEFTSHIFT "KEY_${c}" ;;
    [0-9]) send "KEY_${c}" ;;
    " ") send KEY_SPACE ;;
    "-") send KEY_MINUS ;;
    "_") send KEY_LEFTSHIFT KEY_MINUS ;;
    "/") send KEY_SLASH ;;
    "?") send KEY_LEFTSHIFT KEY_SLASH ;;
    "+") send KEY_LEFTSHIFT KEY_EQUAL ;;
    "=") send KEY_EQUAL ;;
    "*") send KEY_LEFTSHIFT KEY_8 ;;
    ".") send KEY_DOT ;;
    ">") send KEY_LEFTSHIFT KEY_DOT ;;
    ",") send KEY_COMMA ;;
    "<") send KEY_LEFTSHIFT KEY_COMMA ;;
    ":") send KEY_LEFTSHIFT KEY_SEMICOLON ;;
    ";") send KEY_SEMICOLON ;;
    "~") send KEY_LEFTSHIFT KEY_GRAVE ;;
    "@") send KEY_LEFTSHIFT KEY_2 ;;
    "\\") send KEY_BACKSLASH ;;
    "|") send KEY_LEFTSHIFT KEY_BACKSLASH ;;
    '"') send KEY_LEFTSHIFT KEY_APOSTROPHE ;;
    "'") send KEY_APOSTROPHE ;;
    "!") send KEY_LEFTSHIFT KEY_1 ;;
    "&") send KEY_LEFTSHIFT KEY_7 ;;
    "(") send KEY_LEFTSHIFT KEY_9 ;;
    ")") send KEY_LEFTSHIFT KEY_0 ;;
    "\$") send KEY_LEFTSHIFT KEY_4 ;;
    "%") send KEY_LEFTSHIFT KEY_5 ;;
    "#") send KEY_LEFTSHIFT KEY_3 ;;
    "^") send KEY_LEFTSHIFT KEY_6 ;;
    "[") send KEY_LEFTBRACE ;;
    "]") send KEY_RIGHTBRACE ;;
    "{") send KEY_LEFTSHIFT KEY_LEFTBRACE ;;
    "}") send KEY_LEFTSHIFT KEY_RIGHTBRACE ;;
    *) echo "vm-type-us.sh: unmapped character '$c', skipped" >&2 ;;
  esac
  sleep 0.06
done
