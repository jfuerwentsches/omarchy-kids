//! Background loop: prunes expired temporary unlocks (re-rendering the
//! launcher tiles when that changes anything) and fires the pre-warning
//! (issue #8) once an active session is within its tier's lead time of a
//! budget or window cutoff.

use crate::state::State;
use crate::{budget, launcher, prewarning};
use chrono::{Datelike, Local, Utc};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TICK_INTERVAL: Duration = Duration::from_secs(20);

pub fn spawn(state: Arc<Mutex<State>>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK_INTERVAL);
        tick(&state);
    });
}

fn tick(state: &Arc<Mutex<State>>) {
    let mut state = state.lock().unwrap();
    let now_utc = Utc::now();

    if state.config.prune_expired_unlocks(now_utc) {
        if let Err(e) = state.config.save(&state.config_path) {
            eprintln!("agentd: failed to save config after unlock expiry: {e:#}");
        }
        if let Err(e) = launcher::render(&state.config) {
            eprintln!("agentd: failed to re-render launcher tiles after unlock expiry: {e:#}");
        }
    }

    let now_local = Local::now();
    let midnight = budget::local_midnight_utc(now_local);
    let lead_minutes = state.config.pre_warning_minutes() as i64;

    let candidates: Vec<(u32, String)> = state
        .active
        .iter()
        .filter(|(_, s)| !s.warned)
        .map(|(pid, s)| (*pid, s.app.clone()))
        .collect();

    for (pid, app) in candidates {
        let mut remaining: Option<i64> = None;
        let mut take_min = |value: i64| {
            remaining = Some(remaining.map_or(value, |r: i64| r.min(value)));
        };

        if let Some(limit) = state.config.daily_total_minutes_today(now_local.weekday()) {
            let used =
                budget::compute_daily_used_minutes(&state.db, &state.active, midnight, now_utc)
                    .unwrap_or(0);
            take_min(limit as i64 - used as i64);
        }
        if let Some(limit) = state.config.per_app_minutes_today(&app, now_local.weekday()) {
            let used = budget::compute_app_used_minutes(
                &state.db,
                &state.active,
                &app,
                midnight,
                now_utc,
            )
            .unwrap_or(0);
            take_min(limit as i64 - used as i64);
        }
        if let Some(mins) = budget::minutes_until_next_window_start(&state.config, now_local) {
            take_min(mins as i64);
        }

        if let Some(remaining) = remaining {
            if remaining <= lead_minutes {
                prewarning::trigger(&app, remaining.max(0) as u32);
                if let Some(session) = state.active.get_mut(&pid) {
                    session.warned = true;
                }
            }
        }
    }
}
