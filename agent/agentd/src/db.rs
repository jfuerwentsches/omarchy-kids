//! SQLite usage log (issue #12). Unlimited retention — see design note
//! "Omarchy Kids - Parental Controls und Bildschirmzeit"; deletion, if ever
//! added, is a Control Center feature, not agentd's.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS usage_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_id TEXT NOT NULL,
            pid INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_seconds INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_usage_sessions_started_at ON usage_sessions(started_at);
        CREATE INDEX IF NOT EXISTS idx_usage_sessions_open ON usage_sessions(app_id, pid, ended_at);

        CREATE TABLE IF NOT EXISTS pin_overrides (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at TEXT NOT NULL,
            app_id TEXT NOT NULL,
            minutes INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS security_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at TEXT NOT NULL,
            event_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            detail TEXT
        );
        ",
    )
    .context("creating schema")?;
    Ok(conn)
}

pub fn start_session(conn: &Connection, app: &str, pid: u32, started_at: DateTime<Utc>) -> Result<()> {
    conn.execute(
        "INSERT INTO usage_sessions (app_id, pid, started_at) VALUES (?1, ?2, ?3)",
        params![app, pid, started_at.to_rfc3339()],
    )?;
    Ok(())
}

/// Closes the most recent still-open session for (app, pid). If none exists
/// (e.g. agentd restarted mid-session and lost its in-memory `active` map),
/// records a zero-duration entry so the stop event isn't silently dropped.
pub fn end_session(conn: &Connection, app: &str, pid: u32, ended_at: DateTime<Utc>) -> Result<()> {
    let open_started_at: Option<String> = conn
        .query_row(
            "SELECT started_at FROM usage_sessions
             WHERE app_id = ?1 AND pid = ?2 AND ended_at IS NULL
             ORDER BY id DESC LIMIT 1",
            params![app, pid],
            |row| row.get(0),
        )
        .optional()?;

    let Some(started_at) = open_started_at else {
        conn.execute(
            "INSERT INTO usage_sessions (app_id, pid, started_at, ended_at, duration_seconds)
             VALUES (?1, ?2, ?3, ?3, 0)",
            params![app, pid, ended_at.to_rfc3339()],
        )?;
        return Ok(());
    };

    let started: DateTime<Utc> = DateTime::parse_from_rfc3339(&started_at)
        .context("parsing stored started_at")?
        .with_timezone(&Utc);
    let duration = (ended_at - started).num_seconds().max(0);

    conn.execute(
        "UPDATE usage_sessions SET ended_at = ?1, duration_seconds = ?2
         WHERE app_id = ?3 AND pid = ?4 AND ended_at IS NULL",
        params![ended_at.to_rfc3339(), duration, app, pid],
    )?;
    Ok(())
}

/// Sum of completed sessions' durations since `since`, optionally filtered
/// to one app. Deliberately excludes still-open sessions — the caller adds
/// their live elapsed time on top (see `budget::compute_*_used_minutes`).
pub fn used_seconds_since(conn: &Connection, app: Option<&str>, since: DateTime<Utc>) -> Result<i64> {
    let seconds = match app {
        Some(app) => conn.query_row(
            "SELECT COALESCE(SUM(duration_seconds), 0) FROM usage_sessions
             WHERE started_at >= ?1 AND ended_at IS NOT NULL AND app_id = ?2",
            params![since.to_rfc3339(), app],
            |row| row.get(0),
        )?,
        None => conn.query_row(
            "SELECT COALESCE(SUM(duration_seconds), 0) FROM usage_sessions
             WHERE started_at >= ?1 AND ended_at IS NOT NULL",
            params![since.to_rfc3339()],
            |row| row.get(0),
        )?,
    };
    Ok(seconds)
}

pub struct AppUsageRow {
    pub app: String,
    pub seconds: i64,
}

pub fn usage_by_app_since(conn: &Connection, since: DateTime<Utc>) -> Result<Vec<AppUsageRow>> {
    let mut stmt = conn.prepare(
        "SELECT app_id, COALESCE(SUM(duration_seconds), 0) FROM usage_sessions
         WHERE started_at >= ?1 AND ended_at IS NOT NULL
         GROUP BY app_id ORDER BY 2 DESC",
    )?;
    let rows = stmt.query_map(params![since.to_rfc3339()], |row| {
        Ok(AppUsageRow {
            app: row.get(0)?,
            seconds: row.get(1)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("reading usage_by_app_since")
}

pub fn insert_pin_override(conn: &Connection, app: &str, minutes: u32, occurred_at: DateTime<Utc>) -> Result<()> {
    conn.execute(
        "INSERT INTO pin_overrides (occurred_at, app_id, minutes) VALUES (?1, ?2, ?3)",
        params![occurred_at.to_rfc3339(), app, minutes],
    )?;
    Ok(())
}

pub fn pin_override_count_since(conn: &Connection, since: DateTime<Utc>) -> Result<u32> {
    conn.query_row(
        "SELECT COUNT(*) FROM pin_overrides WHERE occurred_at >= ?1",
        params![since.to_rfc3339()],
        |row| row.get(0),
    )
    .context("counting pin overrides")
}

pub fn insert_security_event(
    conn: &Connection,
    event_type: &str,
    severity: &str,
    detail: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO security_events (occurred_at, event_type, severity, detail) VALUES (?1, ?2, ?3, ?4)",
        params![occurred_at.to_rfc3339(), event_type, severity, detail],
    )?;
    Ok(())
}

pub struct SecurityEventRow {
    pub occurred_at: String,
    pub event_type: String,
    pub severity: String,
    pub detail: Option<String>,
}

pub fn recent_security_events(conn: &Connection, since: DateTime<Utc>) -> Result<Vec<SecurityEventRow>> {
    let mut stmt = conn.prepare(
        "SELECT occurred_at, event_type, severity, detail FROM security_events
         WHERE occurred_at >= ?1 ORDER BY occurred_at DESC",
    )?;
    let rows = stmt.query_map(params![since.to_rfc3339()], |row| {
        Ok(SecurityEventRow {
            occurred_at: row.get(0)?,
            event_type: row.get(1)?,
            severity: row.get(2)?,
            detail: row.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("reading recent_security_events")
}

/// Failed-override attempts for one app within a trailing window — feeds the
/// "gehäufte PIN-Fehlversuche" severity threshold (issue #10).
pub fn recent_override_failure_count(conn: &Connection, app: &str, since: DateTime<Utc>) -> Result<u32> {
    conn.query_row(
        "SELECT COUNT(*) FROM security_events
         WHERE event_type = 'override_failed' AND detail = ?1 AND occurred_at >= ?2",
        params![app, since.to_rfc3339()],
        |row| row.get(0),
    )
    .context("counting recent override failures")
}
