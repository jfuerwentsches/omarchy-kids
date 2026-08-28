use chrono::{DateTime, Utc};
use omarchy_kids_common::config::Config;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct State {
    pub config: Config,
    pub config_path: PathBuf,
    pub db: Connection,
    /// Keyed by pid rather than app id — nothing stops two wrapper instances
    /// of different apps running at once, and pid is what `WrapperStop`/
    /// `WrapperPoll` identify a session by.
    pub active: HashMap<u32, ActiveSession>,
}

pub struct ActiveSession {
    pub app: String,
    pub started_at: DateTime<Utc>,
    /// Whether the pre-warning has already fired for this session, so the
    /// ticker doesn't re-trigger it every 20s once the lead-time threshold
    /// is crossed.
    pub warned: bool,
}
