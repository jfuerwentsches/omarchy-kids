use anyhow::{Context, Result};
use chrono::{DateTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// `~/.config/omarchy-kids/config.toml` — the policy the parent sets (tier,
/// time budgets/windows, base app list) plus the temporary-unlock state
/// agentd itself writes back. See design note "Omarchy Kids - Implementierung
/// Agent" > "Datenmodell (Entwurf)".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub tier: TierConfig,
    #[serde(default)]
    pub time_budget: TimeBudgetConfig,
    /// Weekday key ("mon".."sun") -> blocked windows for that day.
    #[serde(default)]
    pub time_windows: BTreeMap<String, Vec<Window>>,
    #[serde(default)]
    pub apps: AppsConfig,
    #[serde(default)]
    pub pre_warning: PreWarningConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    pub current: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimeBudgetConfig {
    /// Weekday key -> total minutes allowed that day, device-wide.
    #[serde(default)]
    pub daily_total_minutes: BTreeMap<String, u32>,
    /// App id -> weekday key -> minutes allowed for that app that day.
    #[serde(default)]
    pub per_app_minutes: BTreeMap<String, BTreeMap<String, u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    /// "HH:MM", local time. `end < start` means the window wraps past midnight
    /// (e.g. start "19:00" end "07:00" for an overnight downtime block).
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppsConfig {
    /// The tier's default tile list, mirrored here by `omarchy-kids-set-tier`
    /// from `tiers/<tier>/launcher/apps.json` at tier-switch time — agentd
    /// needs this to re-render `launcher-apps.json` (base + temporary
    /// unlocks) without depending on the tiers/ source tree at runtime.
    #[serde(default)]
    pub base: Vec<AppTile>,
    /// Temporary unlocks granted via `agent unlock` / `agent override`.
    #[serde(default)]
    pub unlocked: Vec<UnlockedApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppTile {
    /// Accepts the tier source tree's `apps.json` shape (`desktopId`)
    /// directly on read, via `agent set-tier --apps-file` — see issue #3.
    #[serde(alias = "desktopId")]
    pub desktop_id: String,
    pub label: String,
    pub icon: String,
    pub swatch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockedApp {
    pub desktop_id: String,
    pub label: String,
    pub icon: String,
    pub swatch: String,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub via_override: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreWarningConfig {
    /// Tier id -> lead time in minutes before a budget/window cutoff.
    /// Concrete per-tier values are still an open design point (vault note
    /// "Noch offene Punkte"); `default_for_tier` below picks a starting value.
    #[serde(default)]
    pub lead_minutes: BTreeMap<String, u32>,
}

/// JSON tile shape consumed by the launcher plugin
/// (`tiers/mini/launcher/omarchy-kids.launcher/Launcher.qml`) — camelCase,
/// unlike the snake_case TOML config, to match its existing `apps.json` shape.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherTile {
    pub desktop_id: String,
    pub label: String,
    pub icon: String,
    pub swatch: String,
}

impl From<&AppTile> for LauncherTile {
    fn from(t: &AppTile) -> Self {
        LauncherTile {
            desktop_id: t.desktop_id.clone(),
            label: t.label.clone(),
            icon: t.icon.clone(),
            swatch: t.swatch.clone(),
        }
    }
}

pub fn weekday_key(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

impl Config {
    /// Loads the config, or writes and returns a fresh default if none exists
    /// yet — lets agentd start on a box the setup wizard/Control Center
    /// hasn't configured yet instead of refusing to run.
    pub fn load_or_init(path: &Path, default_tier: &str) -> Result<Config> {
        if path.exists() {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let cfg: Config =
                toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
            Ok(cfg)
        } else {
            let cfg = Config::default_for_tier(default_tier);
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    pub fn default_for_tier(tier: &str) -> Config {
        let mut lead_minutes = BTreeMap::new();
        // 5-7 (mini): non-readers, short lead time on an acoustic cue (see
        // "Omarchy Kids - Parental Controls und Bildschirmzeit").
        lead_minutes.insert("mini".to_string(), 1);

        Config {
            tier: TierConfig {
                current: tier.to_string(),
            },
            time_budget: TimeBudgetConfig::default(),
            time_windows: BTreeMap::new(),
            apps: AppsConfig::default(),
            pre_warning: PreWarningConfig { lead_minutes },
        }
    }

    /// Atomic write (tmp file + rename) so a reader never observes a
    /// half-written file — agentd rewrites this on every unlock/expiry.
    pub fn save(&self, path: &Path) -> Result<()> {
        let raw = toml::to_string_pretty(self).context("serializing config")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let tmp_path = path.with_extension("toml.tmp");
        std::fs::write(&tmp_path, raw)
            .with_context(|| format!("writing {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("renaming into {}", path.display()))?;
        Ok(())
    }

    /// Drops unlocks whose expiry has passed. Returns whether anything changed
    /// (callers use this to decide whether launcher-apps.json needs re-rendering).
    pub fn prune_expired_unlocks(&mut self, now: DateTime<Utc>) -> bool {
        let before = self.apps.unlocked.len();
        self.apps.unlocked.retain(|u| u.expires_at > now);
        self.apps.unlocked.len() != before
    }

    /// Base tiles plus currently-active temporary unlocks, de-duplicated by
    /// desktop id (a temporary unlock of an already-base app is a no-op tile
    /// but still extends what the wrapper/budget logic treats as allowed).
    pub fn effective_launcher_tiles(&self) -> Vec<LauncherTile> {
        let mut seen = std::collections::HashSet::new();
        let mut tiles = Vec::new();
        for tile in &self.apps.base {
            if seen.insert(tile.desktop_id.clone()) {
                tiles.push(LauncherTile::from(tile));
            }
        }
        for unlocked in &self.apps.unlocked {
            if seen.insert(unlocked.desktop_id.clone()) {
                tiles.push(LauncherTile {
                    desktop_id: unlocked.desktop_id.clone(),
                    label: unlocked.label.clone(),
                    icon: unlocked.icon.clone(),
                    swatch: unlocked.swatch.clone(),
                });
            }
        }
        tiles
    }

    pub fn is_app_allowed(&self, desktop_id: &str) -> bool {
        self.apps.base.iter().any(|t| t.desktop_id == desktop_id)
            || self
                .apps
                .unlocked
                .iter()
                .any(|u| u.desktop_id == desktop_id)
    }

    pub fn daily_total_minutes_today(&self, weekday: Weekday) -> Option<u32> {
        self.time_budget
            .daily_total_minutes
            .get(weekday_key(weekday))
            .copied()
    }

    pub fn per_app_minutes_today(&self, app: &str, weekday: Weekday) -> Option<u32> {
        self.time_budget
            .per_app_minutes
            .get(app)
            .and_then(|w| w.get(weekday_key(weekday)))
            .copied()
    }

    pub fn windows_today(&self, weekday: Weekday) -> &[Window] {
        self.time_windows
            .get(weekday_key(weekday))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn pre_warning_minutes(&self) -> u32 {
        self.pre_warning
            .lead_minutes
            .get(&self.tier.current)
            .copied()
            .unwrap_or(2)
    }
}
