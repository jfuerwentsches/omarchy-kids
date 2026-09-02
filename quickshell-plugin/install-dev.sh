#!/bin/bash
# Dev-only installer for the parent-computer headerbar plugin, for testing
# on the developer's own live Omarchy desktop. It reuses the same helper
# that the package installs, but points it at the repo checkout instead of
# /usr/share/omarchy-kids/... : third-party plugins are opt-in in Omarchy
# (dropping the directory into ~/.config/omarchy/plugins/ alone does
# nothing), and a bar-widget specifically has to be placed in shell.json's
# bar.layout.<section>, not just plugins[] (see PluginRegistry.qml's
# setEnabled() — plugins[] is where non-bar-widget kinds like
# overlays/menus register, a bar widget's real placement is bar.layout).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OMARCHY_KIDS_QUICKSHELL_PLUGIN_SOURCE_DIR="$SCRIPT_DIR/omarchy-kids.control" \
  "$SCRIPT_DIR/omarchy-kids-quickshell-plugin-enable"
