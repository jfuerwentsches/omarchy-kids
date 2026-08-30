#!/bin/bash
# Dev-only installer for the parent-computer headerbar plugin, for testing
# on the developer's own live Omarchy desktop. There is no packaged install
# path yet (control/ isn't packaged at all — see root CLAUDE.md "Status"),
# so this mirrors tiers/omarchy-kids-set-tier's own plugin-install steps by
# hand: third-party plugins are opt-in in Omarchy (dropping the directory
# into ~/.config/omarchy/plugins/ alone does nothing), and a bar-widget
# specifically has to be placed in shell.json's bar.layout.<section>, not
# just plugins[] (see PluginRegistry.qml's setEnabled() — plugins[] is
# where non-bar-widget kinds like overlays/menus register, a bar widget's
# real placement is bar.layout).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="$SCRIPT_DIR/omarchy-kids.control"
PLUGIN_ID="omarchy-kids.control"

mkdir -p "$HOME/.config/omarchy/plugins"
rm -rf "$HOME/.config/omarchy/plugins/$PLUGIN_ID"
cp -r "$PLUGIN_DIR" "$HOME/.config/omarchy/plugins/$PLUGIN_ID"

SHELL_JSON="$HOME/.config/omarchy/shell.json"
[[ -f $SHELL_JSON ]] || echo '{}' > "$SHELL_JSON"

tmp_json="$(mktemp)"
jq --arg id "$PLUGIN_ID" '
  .bar.layout.right = ((.bar.layout.right // []) | if any(.id == $id) then . else . + [{id: $id}] end)
' "$SHELL_JSON" > "$tmp_json" && mv "$tmp_json" "$SHELL_JSON"

# Same reload trick as omarchy-kids-set-tier: Quickshell's own live plugin
# reload has been observed hanging onto a stale layout after a plugin's
# first install, a full restart is more reliable. It respawns via Hyprland
# autostart.
pkill -f 'quickshell -n -p' >/dev/null 2>&1 || true

echo "Installed $PLUGIN_ID into $HOME/.config/omarchy/plugins and shell.json's bar.layout.right."
