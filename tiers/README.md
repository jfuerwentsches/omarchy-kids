# tiers

Age-tier config layer. Each tier is a config profile swapping Hyprland config, Quickshell modules (launcher, status bar), and a theme (see `omarchy-kids-themes` design note) — wallpaper, colors, scaling.

Entry point: `omarchy-kids-set-tier <tier>` (e.g. `omarchy-kids-set-tier 5-7`). Installs `<tier>/theme/` as an Omarchy theme (`~/.config/omarchy/themes/omarchy-kids-<tier>`) and activates it via `omarchy-theme-set`.

## Status

- `5-7/theme/`: first working theme ("Sternenreise" — original space artwork, not tied to any licensed franchise). High-contrast palette, larger UI scale/font for small hands and pre-readers.
- Hyprland config swap and the per-tier Quickshell launcher module (kiosk icon grid for 5-7, see the age-tier concept notes) are not implemented yet — `omarchy-kids-set-tier` currently only swaps the theme.
- Other tiers (8-10, 11-13, 14-16) not started.
