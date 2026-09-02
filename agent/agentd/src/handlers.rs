use crate::{allowlist, budget, db, launcher, security};
use crate::state::{ActiveSession, State};
use chrono::{Datelike, Duration as ChronoDuration, Local, Utc};
use omarchy_kids_common::config::UnlockedApp;
use omarchy_kids_common::desktop;
use omarchy_kids_common::paths;
use omarchy_kids_common::protocol::{
    AllowedPayload, AppUsage, DEFAULT_UNLOCK_MINUTES, Request, Response, ReportPayload,
    SecurityEventSummary, StatusPayload,
};
use std::sync::{Arc, Mutex};

/// Failed override attempts for the same app within a 5-minute window before
/// the event escalates from "routine" to "severe" (issue #10's open
/// threshold question — 3 chosen to match a typical "wrong PIN" lockout
/// convention without being so low that a genuine typo trips it).
const SEVERE_OVERRIDE_FAILURE_THRESHOLD: u32 = 3;
const OVERRIDE_FAILURE_WINDOW_MINUTES: i64 = 5;

/// Tiers agentd will accept for `SetTier` (issue #32). `SetTier`'s only real
/// caller (`omarchy-kids-set-tier`, invoked via `sudo -u <child>` — see
/// `setup-wizard/bootstrap/lib/branding-tier.sh`) runs at the child's own
/// uid, same as any other local process, so the socket can't distinguish a
/// legitimate tier switch from a malicious one by caller identity alone.
/// This enumeration plus the per-tile validation below is the accepted
/// mitigation: content is validated, the caller isn't. Matches CLAUDE.md's
/// "Current focus" — extend this when a second tier actually ships.
const KNOWN_TIERS: &[&str] = &["mini"];

pub fn dispatch(state: &Arc<Mutex<State>>, req: Request) -> Response {
    match req {
        Request::Ping => Response::ok_empty(),
        Request::Status => handle_status(state),
        Request::SetTier { tier, apps } => handle_set_tier(state, tier, apps),
        Request::Unlock { app, minutes } => handle_unlock(state, app, minutes, false),
        Request::Override { app, minutes } => handle_unlock(state, app, minutes, true),
        Request::OverrideFailed { app } => handle_override_failed(state, app),
        Request::Report { week } => handle_report(state, week),
        Request::WrapperAuthorize { app } => handle_wrapper_authorize(state, app),
        Request::WrapperStart { app, pid } => handle_wrapper_start(state, app, pid),
        Request::WrapperStop {
            app,
            pid,
            exit_code: _,
        } => handle_wrapper_stop(state, app, pid),
        Request::WrapperPoll { app, pid } => handle_wrapper_poll(state, app, pid),
        Request::PushContent { .. } => {
            Response::err("push-content is reserved but not implemented yet (see issue #13)")
        }
    }
}

fn handle_status(state: &Arc<Mutex<State>>) -> Response {
    let state = state.lock().unwrap();
    let now_local = Local::now();
    let now_utc = Utc::now();
    let midnight = budget::local_midnight_utc(now_local);

    let used = budget::compute_daily_used_minutes(&state.db, &state.active, midnight, now_utc)
        .unwrap_or(0);
    let budget_minutes = state
        .config
        .daily_total_minutes_today(now_local.weekday())
        .unwrap_or(0);
    let remaining = budget_minutes.saturating_sub(used);
    let active_app = state.active.values().next().map(|s| s.app.clone());
    let unlocked_apps = state
        .config
        .effective_launcher_tiles()
        .into_iter()
        .map(|t| t.desktop_id)
        .collect();

    Response::ok_with(StatusPayload {
        tier: state.config.tier.current.clone(),
        unlocked_apps,
        daily_budget_minutes: budget_minutes,
        daily_used_minutes: used,
        daily_remaining_minutes: remaining,
        in_blocked_window: budget::in_blocked_window(&state.config, now_local),
        active_app,
    })
}

fn handle_set_tier(
    state: &Arc<Mutex<State>>,
    tier: String,
    apps: Option<Vec<omarchy_kids_common::config::AppTile>>,
) -> Response {
    if !KNOWN_TIERS.contains(&tier.as_str()) {
        return Response::err(format!("unknown tier '{tier}'"));
    }
    if let Some(apps) = &apps {
        for tile in apps {
            if desktop::find_desktop_file(&tile.desktop_id).is_none() {
                return Response::err(format!(
                    "refusing to set tier: '{}' does not resolve to an installed application",
                    tile.desktop_id
                ));
            }
        }
    }

    let mut state = state.lock().unwrap();
    state.config.tier.current = tier;
    if let Some(apps) = apps {
        state.config.apps.base = apps;
    }

    if let Err(e) = state.config.save(&state.config_path) {
        return Response::err(format!("failed to save config: {e:#}"));
    }
    if let Err(e) = launcher::render(&state.config) {
        return Response::err(format!("failed to render launcher tiles: {e:#}"));
    }
    Response::ok_empty()
}

/// Looks up a nicer label/icon for a temporarily-unlocked app from its
/// `.desktop` entry; falls back to the raw id if it can't be found (still a
/// usable tile, just less pretty).
fn tile_metadata_for(desktop_id: &str) -> (String, String, String) {
    const FALLBACK_SWATCH: &str = "#94A3B8";
    if let Some(path) = desktop::find_desktop_file(desktop_id) {
        if let Ok(entry) = desktop::parse_desktop_entry(&path) {
            return (entry.name, entry.icon, FALLBACK_SWATCH.to_string());
        }
    }
    (
        desktop_id.to_string(),
        desktop_id.to_string(),
        FALLBACK_SWATCH.to_string(),
    )
}

/// `Unlock` (unauthenticated at the socket layer) may only grant an app on
/// the parent-curated allow-list; `Override` is exempt because it's only
/// ever sent after a live polkit admin authentication has already happened
/// (see `agent/agent/src/main.rs`'s `run_override`) — a stronger, in-the-
/// moment authorization the allow-list doesn't need to duplicate.
fn unlock_is_permitted(app: &str, via_override: bool, allowlist: &[String]) -> bool {
    via_override || allowlist.iter().any(|a| a == app)
}

fn handle_unlock(
    state: &Arc<Mutex<State>>,
    app: String,
    minutes: Option<u32>,
    via_override: bool,
) -> Response {
    let allowed_apps = allowlist::load(&paths::unlockable_apps_path());
    if !unlock_is_permitted(&app, via_override, &allowed_apps) {
        return Response::err(format!(
            "'{app}' is not on the parent-curated unlockable-apps list ({})",
            paths::unlockable_apps_path().display()
        ));
    }

    let minutes = minutes.unwrap_or(DEFAULT_UNLOCK_MINUTES);
    let (label, icon, swatch) = tile_metadata_for(&app);

    let mut state = state.lock().unwrap();
    let now = Utc::now();
    let expires_at = now + ChronoDuration::minutes(minutes as i64);

    state.config.apps.unlocked.retain(|u| u.desktop_id != app);
    state.config.apps.unlocked.push(UnlockedApp {
        desktop_id: app.clone(),
        label,
        icon,
        swatch,
        expires_at,
        via_override,
    });

    if let Err(e) = state.config.save(&state.config_path) {
        return Response::err(format!("failed to save config: {e:#}"));
    }
    if let Err(e) = launcher::render(&state.config) {
        return Response::err(format!("failed to render launcher tiles: {e:#}"));
    }
    if via_override {
        if let Err(e) = db::insert_pin_override(&state.db, &app, minutes, now) {
            eprintln!("agentd: failed to log pin override: {e:#}");
        }
    }
    Response::ok_empty()
}

fn handle_override_failed(state: &Arc<Mutex<State>>, app: String) -> Response {
    let state = state.lock().unwrap();
    let now = Utc::now();
    let window_start = now - ChronoDuration::minutes(OVERRIDE_FAILURE_WINDOW_MINUTES);

    let recent = db::recent_override_failure_count(&state.db, &app, window_start).unwrap_or(0);
    let severity = if recent + 1 >= SEVERE_OVERRIDE_FAILURE_THRESHOLD {
        "severe"
    } else {
        "routine"
    };

    if let Err(e) = db::insert_security_event(&state.db, "override_failed", severity, Some(&app), now) {
        return Response::err(format!("failed to log security event: {e:#}"));
    }
    if severity == "severe" {
        security::notify_severe(&format!(
            "Repeated failed unlock attempts for '{app}' ({} in the last {OVERRIDE_FAILURE_WINDOW_MINUTES} min)",
            recent + 1
        ));
    }
    Response::ok_empty()
}

fn handle_wrapper_start(state: &Arc<Mutex<State>>, app: String, pid: u32) -> Response {
    let mut state = state.lock().unwrap();
    let now = Utc::now();

    if !state.config.is_app_allowed(&app) {
        // Not fatal — still record the session for transparency in the
        // report; the next WrapperPoll (within POLL_INTERVAL) will tell the
        // wrapper to terminate it.
        let _ = db::insert_security_event(
            &state.db,
            "wrapper_started_disallowed_app",
            "routine",
            Some(&app),
            now,
        );
    }

    if let Err(e) = db::start_session(&state.db, &app, pid, now) {
        return Response::err(format!("failed to log session start: {e:#}"));
    }
    state.active.insert(
        pid,
        ActiveSession {
            app,
            started_at: now,
            warned: false,
        },
    );
    Response::ok_empty()
}

fn handle_wrapper_stop(state: &Arc<Mutex<State>>, app: String, pid: u32) -> Response {
    let mut state = state.lock().unwrap();
    let now = Utc::now();
    state.active.remove(&pid);
    if let Err(e) = db::end_session(&state.db, &app, pid, now) {
        return Response::err(format!("failed to log session stop: {e:#}"));
    }
    Response::ok_empty()
}

/// Shared by `WrapperAuthorize` (checked *before* a launch, issue #35) and
/// `WrapperPoll` (checked periodically while already running) — same
/// allow-list/budget/window rules apply either way, only the pid-keyed
/// session accounting differs (and neither of these actually needs the pid).
fn check_allowed(state: &State, app: &str) -> AllowedPayload {
    let now_utc = Utc::now();
    let now_local = Local::now();

    let disallow = |reason: &str| AllowedPayload {
        allowed: false,
        reason: Some(reason.to_string()),
    };

    if !state.config.is_app_allowed(app) {
        return disallow("app is not (or no longer) unlocked");
    }
    if budget::in_blocked_window(&state.config, now_local) {
        // Time windows get no grace buffer — hard cutoff, per design note.
        return disallow("currently inside a blocked time window");
    }

    let midnight = budget::local_midnight_utc(now_local);
    if let Some(limit) = state
        .config
        .daily_total_minutes_today(now_local.weekday())
    {
        let used =
            budget::compute_daily_used_minutes(&state.db, &state.active, midnight, now_utc)
                .unwrap_or(0);
        if used > limit + budget::GRACE_MINUTES {
            return disallow("daily time budget exhausted");
        }
    }
    if let Some(limit) = state.config.per_app_minutes_today(app, now_local.weekday()) {
        let used =
            budget::compute_app_used_minutes(&state.db, &state.active, app, midnight, now_utc)
                .unwrap_or(0);
        if used > limit + budget::GRACE_MINUTES {
            return disallow("per-app time budget exhausted");
        }
    }

    AllowedPayload {
        allowed: true,
        reason: None,
    }
}

fn handle_wrapper_authorize(state: &Arc<Mutex<State>>, app: String) -> Response {
    let state = state.lock().unwrap();
    Response::ok_with(check_allowed(&state, &app))
}

fn handle_wrapper_poll(state: &Arc<Mutex<State>>, app: String, _pid: u32) -> Response {
    let state = state.lock().unwrap();
    Response::ok_with(check_allowed(&state, &app))
}

fn handle_report(state: &Arc<Mutex<State>>, week: bool) -> Response {
    let state = state.lock().unwrap();
    let range_days: i64 = if week { 7 } else { 1 };
    let now = Utc::now();
    let since = now - ChronoDuration::days(range_days);
    let prev_since = now - ChronoDuration::days(range_days * 2);

    let per_app_rows = match db::usage_by_app_since(&state.db, since) {
        Ok(v) => v,
        Err(e) => return Response::err(format!("failed to read usage: {e:#}")),
    };
    let per_app: Vec<AppUsage> = per_app_rows
        .into_iter()
        .map(|r| AppUsage {
            app: r.app,
            minutes: (r.seconds / 60) as u32,
        })
        .collect();

    let total_this = match db::used_seconds_since(&state.db, None, since) {
        Ok(v) => (v / 60) as u32,
        Err(e) => return Response::err(format!("failed to sum usage: {e:#}")),
    };
    let total_incl_previous = match db::used_seconds_since(&state.db, None, prev_since) {
        Ok(v) => (v / 60) as u32,
        Err(e) => return Response::err(format!("failed to sum usage: {e:#}")),
    };
    let total_previous = total_incl_previous.saturating_sub(total_this);

    let pin_override_count = db::pin_override_count_since(&state.db, since).unwrap_or(0);
    let security_events = db::recent_security_events(&state.db, since)
        .unwrap_or_default()
        .into_iter()
        .map(|r| SecurityEventSummary {
            occurred_at: r.occurred_at,
            event_type: r.event_type,
            severity: r.severity,
            detail: r.detail,
        })
        .collect();

    Response::ok_with(ReportPayload {
        range_days: range_days as u32,
        per_app,
        total_minutes_this_period: total_this,
        total_minutes_previous_period: total_previous,
        pin_override_count,
        security_events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_rejects_apps_not_on_the_allowlist() {
        let allowed = vec!["org.kde.gcompris".to_string()];
        assert!(!unlock_is_permitted("org.gnome.Terminal", false, &allowed));
        assert!(!unlock_is_permitted("org.gnome.Terminal", false, &[]));
    }

    #[test]
    fn unlock_permits_apps_on_the_allowlist() {
        let allowed = vec!["org.kde.gcompris".to_string()];
        assert!(unlock_is_permitted("org.kde.gcompris", false, &allowed));
    }

    #[test]
    fn override_bypasses_the_allowlist() {
        // Override is only ever sent after a live polkit admin
        // authentication (see run_override in agent/agent/src/main.rs) — a
        // stronger, in-the-moment check the allow-list doesn't need to gate.
        assert!(unlock_is_permitted("anything", true, &[]));
    }

    #[test]
    fn known_tiers_rejects_unrecognized_names() {
        assert!(!KNOWN_TIERS.contains(&"midi"));
        assert!(KNOWN_TIERS.contains(&"mini"));
    }
}
