//! Renders `~/.config/omarchy-kids/launcher-apps.json` from the config's
//! base tile list plus active temporary unlocks. The launcher plugin
//! (`tiers/mini/launcher/omarchy-kids.launcher/Launcher.qml`) watches this
//! file (`FileView { watchChanges: true }`) and picks up changes live — no
//! signal/restart needed on our side.

use anyhow::{Context, Result};
use omarchy_kids_common::{config::Config, paths};

pub fn render(config: &Config) -> Result<()> {
    let tiles = config.effective_launcher_tiles();
    let json = serde_json::to_string_pretty(&tiles).context("serializing launcher tiles")?;

    let path = paths::launcher_apps_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, json)
        .with_context(|| format!("writing {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &path)
        .with_context(|| format!("renaming into {}", path.display()))?;
    Ok(())
}
