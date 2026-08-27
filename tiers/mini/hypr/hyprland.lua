-- Omarchy Kids: Tier mini (5-7, kiosk). Installed by omarchy-kids-set-tier as a
-- full replacement for ~/.config/hypr/hyprland.lua — edit
-- tiers/mini/hypr/hyprland.lua in the omarchy-kids repo, not this file
-- directly. The original is backed up as hyprland.lua.pre-omarchy-kids.

dofile((os.getenv("OMARCHY_PATH") or "/usr/share/omarchy") .. "/default/hypr/bootstrap.lua")

-- Kiosk mode: no terminal, no file manager, no window/workspace management,
-- no webapp shortcuts — a 5-7 year old should never be able to reach
-- anything but the launcher and whatever app it opens. The only surviving
-- gesture is SUPER + SPACE, rebound below to the Kids kiosk launcher
-- instead of the normal Omarchy menu, so the "SUPER opens app access"
-- muscle memory carries into later tiers unchanged.
omarchy_default_bindings = false
omarchy_preinstalled_bindings = false

require("default.hypr.omarchy")
require("hypr.monitors")
require("hypr.input")
require("hypr.looknfeel")
require("hypr.autostart")
require("default.hypr.toggles")

o.bind("SUPER + SPACE", "Kids launcher", "omarchy-shell shell toggle omarchy-kids.launcher")
