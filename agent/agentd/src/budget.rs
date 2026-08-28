//! Time-budget and time-window logic (issue #7). Pure functions over
//! `Config` + the usage db + the live `active` sessions map, kept separate
//! from `handlers.rs` so the cutoff rules can be reasoned about (and tested)
//! without the socket/dispatch machinery.

use crate::db;
use crate::state::ActiveSession;
use anyhow::Result;
use chrono::{DateTime, Datelike, Local, NaiveTime, TimeZone, Utc};
use omarchy_kids_common::config::{Config, Window};
use rusqlite::Connection;
use std::collections::HashMap;

/// "one extra minute, once, without prompting" — see design note
/// "Omarchy Kids - Parental Controls und Bildschirmzeit" > "Durchsetzung bei
/// Limit-Erreichen". Applies to daily/app limits only; time windows get no
/// grace (hard cutoff), per the same note.
pub const GRACE_MINUTES: u32 = 1;

pub fn local_midnight_utc(now_local: DateTime<Local>) -> DateTime<Utc> {
    let midnight_naive = now_local
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("00:00:00 is always valid");
    Local
        .from_local_datetime(&midnight_naive)
        .single()
        .unwrap_or(now_local)
        .with_timezone(&Utc)
}

pub fn compute_daily_used_minutes(
    conn: &Connection,
    active: &HashMap<u32, ActiveSession>,
    since_midnight_utc: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<u32> {
    let completed = db::used_seconds_since(conn, None, since_midnight_utc)?;
    let live: i64 = active
        .values()
        .filter(|s| s.started_at >= since_midnight_utc)
        .map(|s| (now - s.started_at).num_seconds().max(0))
        .sum();
    Ok(((completed + live) / 60) as u32)
}

pub fn compute_app_used_minutes(
    conn: &Connection,
    active: &HashMap<u32, ActiveSession>,
    app: &str,
    since_midnight_utc: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<u32> {
    let completed = db::used_seconds_since(conn, Some(app), since_midnight_utc)?;
    let live: i64 = active
        .values()
        .filter(|s| s.app == app && s.started_at >= since_midnight_utc)
        .map(|s| (now - s.started_at).num_seconds().max(0))
        .sum();
    Ok(((completed + live) / 60) as u32)
}

fn parse_time(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M").ok()
}

/// The part of a (possibly overnight) window that falls on its own keyed
/// weekday: for a same-day window that's the whole thing; for one that wraps
/// past midnight, only the "start until midnight" half.
fn covers_from_start(w: &Window, time: NaiveTime) -> bool {
    let (Some(start), Some(end)) = (parse_time(&w.start), parse_time(&w.end)) else {
        return false;
    };
    if start <= end {
        time >= start && time < end
    } else {
        time >= start
    }
}

/// The "midnight until end" half of yesterday's window, if it wraps —
/// evaluated against *today's* time so an overnight downtime block keyed to
/// e.g. "mon" still blocks early Tuesday morning.
fn covers_until_end_wrap(w: &Window, time: NaiveTime) -> bool {
    let (Some(start), Some(end)) = (parse_time(&w.start), parse_time(&w.end)) else {
        return false;
    };
    start > end && time < end
}

pub fn in_blocked_window(config: &Config, now_local: DateTime<Local>) -> bool {
    let weekday = now_local.weekday();
    let time = now_local.time();
    if config
        .windows_today(weekday)
        .iter()
        .any(|w| covers_from_start(w, time))
    {
        return true;
    }
    config
        .windows_today(weekday.pred())
        .iter()
        .any(|w| covers_until_end_wrap(w, time))
}

/// Minutes until the next window *starts* today (for the pre-warning lead
/// time) — `None` if no more windows start today.
pub fn minutes_until_next_window_start(config: &Config, now_local: DateTime<Local>) -> Option<u32> {
    let weekday = now_local.weekday();
    let time = now_local.time();
    config
        .windows_today(weekday)
        .iter()
        .filter_map(|w| parse_time(&w.start))
        .filter(|start| *start > time)
        .map(|start| (start - time).num_minutes().max(0) as u32)
        .min()
}
