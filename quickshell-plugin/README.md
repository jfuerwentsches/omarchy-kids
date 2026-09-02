# quickshell-plugin

Parent-computer headerbar plugin (QML, Quickshell's plugin mechanism, kind
`bar-widget`). Lives at `omarchy-kids.control/` — see its `manifest.json`.

- A bar icon (child glyph) that turns the bar's "needs attention" color when
  any paired child is offline (`BarIconButton`'s `active`/`activeColor`).
- Click opens a popup (`qs.Ui`'s `Panel`/`KeyboardPanel`, same mechanism as
  Omarchy's own Dropbox/Clock bar widgets) listing every paired child with
  an online/offline dot, plus an "Open Control Center" row. Read from
  `~/.config/omarchy-kids-control/status-cache.json` via a `FileView` —
  never speaks SSH itself, see the trust-boundary decision in `Omarchy Kids
  - Implementierung Control Center`. That cache is written by the headless
  `omarchy-kids-control --poll` (see `control/core/`'s
  `poll_runner`/`status_cache`), meant to run periodically via
  `control/packaging/systemd/omarchy-kids-control-poll.timer`.
- The popup's "Open Control Center" row (and a middle-click on the icon)
  launches `omarchy-kids-control` (`Quickshell.execDetached`) — the full
  GUI, whether or not a child is paired yet. Pairing itself (mDNS/QR
  discovery, SPAKE2 confirmation) lives entirely in that GUI's
  `PairingDialog`; this widget has no copy of that flow, it's just the
  entry point into it.
- Also reachable via `omarchy-shell omarchy-kids.control open/close/toggle`
  (the `Panel` base's IPC handler, `ipcTarget: "omarchy-kids.control"`).

Packaged as `omarchy-kids-quickshell-plugin`. The installed
`omarchy-kids-quickshell-plugin-enable` helper copies the plugin into
`~/.config/omarchy/plugins/` and adds it to `shell.json`'s
`bar.layout.right` (a bar-widget's real placement - unlike overlay/menu
plugins, being listed in top-level `plugins[]` alone doesn't render it in
the bar, see `PluginRegistry.qml`'s `setEnabled()`), then restarts
Quickshell.

The systemd timer still lives with `control/`'s package; until that package
is installed, the cache is only as fresh as the last manual
`omarchy-kids-control --poll`.
