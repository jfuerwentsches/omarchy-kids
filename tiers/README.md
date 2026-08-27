# tiers

Age-tier config layer. Each tier is a config profile swapping Hyprland config, Quickshell modules (launcher, status bar), and a theme (see `omarchy-kids-themes` design note) — wallpaper, colors, scaling.

Entry point: `omarchy-kids-set-tier <tier>` (e.g. `omarchy-kids-set-tier 5-7`). Installs `<tier>/theme/` as an Omarchy theme (`~/.config/omarchy/themes/omarchy-kids-<tier>`) and activates it via `omarchy-theme-set`.

## Status

Tier 5-7 is a working end-to-end kiosk, verified in the dev VM:

- `5-7/theme/`: "Sternenreise" — original space artwork, not tied to any licensed franchise. High-contrast palette, larger UI scale/font for small hands and pre-readers.
- `5-7/launcher/`: `omarchy-kids.launcher`, a Quickshell overlay plugin — fullscreen icon grid, no text/reading required to use it. `apps.json` currently lists GCompris, Tux Paint, and Blinken (installed via `pacman`/AUR, not part of this package yet — see open points).
- `5-7/hypr/hyprland.lua`: full replacement of the user's Hyprland config. Disables every default Omarchy binding (`omarchy_default_bindings = false`, `omarchy_preinstalled_bindings = false` — no terminal, no file manager, no webapp shortcuts, no tiling/workspace binds) and rebinds only `SUPER + SPACE` to the kiosk launcher, so the "SUPER opens app access" gesture survives into later tiers unchanged. `omarchy-kids-set-tier` backs up the original as `hyprland.lua.pre-omarchy-kids` before overwriting.
- `omarchy-kids-set-tier` also masks `getty@tty2-6` for tier 5-7, so `Ctrl+Alt+F<n>` can't reach a raw login shell for the same account — the kiosk restriction is otherwise only a UI-layer thing (Quickshell config), not an access control, since nothing about it restricts the account itself.

Other tiers (8-10, 11-13, 14-16) not started.

### Open points

- Launcher app set is hardcoded for 5-7 in `apps.json`; no install step for GCompris/Tux Paint/Blinken yet (Tux Paint is AUR-only — see packaging note in the Altersstufe 5-7 concept doc).
- Apps launch directly (`gtk-launch`, same mechanism Omarchy's own app library uses) — no time tracking / app-wrapper integration yet, that lands with `agent`/`agentd`.
- A newly installed launcher plugin can need a full Quickshell restart to render correctly the first time (`pkill -f 'quickshell -n -p'`, it respawns via Hyprland autostart) — `omarchy-kids-set-tier` does this automatically when a tier ships a `launcher/`.
