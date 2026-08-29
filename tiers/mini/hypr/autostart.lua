-- Omarchy Kids: Tier mini (5-7, kiosk) first-run replacement. Installed by
-- omarchy-kids-set-tier as a full replacement for ~/.config/hypr/autostart.lua
-- — edit tiers/mini/hypr/autostart.lua in the omarchy-kids repo, not this
-- file directly. The original is backed up as autostart.lua.pre-omarchy-kids.
--
-- Issue #30: default.hypr.omarchy's own "hyprland.start" handler
-- unconditionally runs `omarchy-provision-first-run`, whose first-login
-- sequence is entirely notification/invitation prompts except for four
-- silent setup steps — and every one of those prompts (Learn Keybindings,
-- Update System, Setup Wi-Fi, plus post-update invitations for Voxtype
-- dictation, fingerprint unlock, and picking a default agent) is aimed at an
-- adult administering the machine, not a non-reading 5-7 year old using a
-- locked-down kiosk account. There is no per-step toggle in
-- omarchy-provision-first-run, only the one all-or-nothing "first-run-user"
-- done-marker.
--
-- omarchy-kids-set-tier marks that done-marker before the child ever logs in
-- graphically for the first time (see tiers/omarchy-kids-set-tier), so
-- Omarchy's own handler always finds it already done and exits immediately —
-- none of the prompts above ever fire. This handler replicates only the four
-- silent steps ourselves, by calling Omarchy's own first-run scripts
-- directly (not duplicating their logic, so they stay in sync with
-- upstream), gated by our own once-only done-marker so they don't re-run
-- every login.
hl.on("hyprland.start", function()
  hl.exec_cmd(
    "omarchy_path=${OMARCHY_PATH:-/usr/share/omarchy}; " ..
    "omarchy-done ensure omarchy-kids-mini-first-run && { " ..
    "bash \"$omarchy_path/install/user/first-run/enable-user-units.sh\"; " ..
    "bash \"$omarchy_path/install/user/first-run/gnome-theme.sh\"; " ..
    "bash \"$omarchy_path/install/user/first-run/gtk-primary-paste.sh\"; " ..
    "bash \"$omarchy_path/install/user/first-run/audio-tuning.sh\"; }"
  )
end)
