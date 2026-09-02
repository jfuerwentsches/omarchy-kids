use serde::{Deserialize, Serialize};

/// Wire format: newline-delimited JSON over a Unix domain socket
/// (`paths::socket_path()`), one request per connection — connect, write one
/// `Request` line, read one `Response` line, close. See docs/agent-protocol.md
/// for the full rationale.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// Liveness check — used by the wrapper's fail-open probe (see issue #6).
    Ping,
    Status,
    /// `apps`, when given, replaces agentd's record of the tier's base tile
    /// list (from `tiers/<tier>/launcher/apps.json`) — agentd is the sole
    /// writer of `launcher-apps.json` from then on (base ++ active temporary
    /// unlocks), so a tier switch doesn't need its own separate ".desktop
    /// wrapper generation" step (issue #15): `omarchy-kids-set-tier` passes
    /// the tier's apps.json straight through here via `--apps-file`.
    SetTier {
        tier: String,
        apps: Option<Vec<crate::config::AppTile>>,
    },
    /// Temporary unlock of an app outside the tier's fixed app list.
    Unlock {
        app: String,
        /// Defaults to `DEFAULT_UNLOCK_MINUTES` when omitted.
        minutes: Option<u32>,
    },
    /// Same effect as `Unlock`, but only accepted from `omarchy-kids-agent
    /// override`, which has already gone through polkit authentication
    /// (`pkexec`) before sending this — see issue #9. Logged separately in
    /// the usage log's `pin_overrides` table for transparency.
    Override {
        app: String,
        minutes: Option<u32>,
    },
    /// Reported by the CLI when `pkexec` itself rejected or the parent
    /// cancelled/mistyped the authentication prompt (pkexec exits 126/127
    /// without ever running the helper, so agentd otherwise never learns an
    /// override was attempted at all). Feeds the security-event detection in
    /// issue #10 ("gehäufte PIN-Fehlversuche").
    OverrideFailed {
        app: String,
    },
    Report {
        week: bool,
    },
    /// Sent by `omarchy-kids-run` **before** it execs anything (issue #35):
    /// answered with `AllowedPayload`, using the same allow/budget/window
    /// checks as `WrapperPoll`. `omarchy-kids-run` fails closed (refuses to
    /// launch) both when this comes back `allowed: false` and when agentd is
    /// unreachable at all — unlike `WrapperStart`/`WrapperStop`/`WrapperPoll`
    /// below, which stay fail-open (agentd being briefly down shouldn't kill
    /// an already-running, already-authorized session).
    WrapperAuthorize {
        app: String,
    },
    /// Sent by `omarchy-kids-run` right after it execs the real app.
    WrapperStart {
        app: String,
        pid: u32,
    },
    /// Sent by `omarchy-kids-run` once the app process exits (normally or
    /// because agentd's enforcement cutoff killed it).
    WrapperStop {
        app: String,
        pid: u32,
        exit_code: Option<i32>,
    },
    /// Polled periodically by `omarchy-kids-run` while the app is running so
    /// agentd has a way to cut a session short (time window hit, budget
    /// exhausted) without needing a reverse connection into the wrapper —
    /// see issue #7. Answered with `AllowedPayload`.
    WrapperPoll {
        app: String,
        pid: u32,
    },
    /// Reserved placeholder (issue #13) for parent-to-child content transfer
    /// (photos, playlist metadata) — not implemented yet. Kept in the enum now
    /// so the wire format doesn't need a second redesign once it lands.
    PushContent {
        content_type: String,
    },
}

pub const DEFAULT_UNLOCK_MINUTES: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok_with(data: impl Serialize) -> Self {
        Response {
            ok: true,
            data: Some(serde_json::to_value(data).expect("payload must serialize")),
            error: None,
        }
    }

    pub fn ok_empty() -> Self {
        Response {
            ok: true,
            data: None,
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Response {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusPayload {
    pub tier: String,
    /// Base tier apps plus any currently-unexpired temporary unlocks.
    pub unlocked_apps: Vec<String>,
    /// 0 means no daily limit is configured for today, not "0 minutes allowed".
    pub daily_budget_minutes: u32,
    pub daily_used_minutes: u32,
    pub daily_remaining_minutes: u32,
    pub in_blocked_window: bool,
    pub active_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedPayload {
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUsage {
    pub app: String,
    pub minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPayload {
    pub range_days: u32,
    pub per_app: Vec<AppUsage>,
    pub total_minutes_this_period: u32,
    pub total_minutes_previous_period: u32,
    pub pin_override_count: u32,
    pub security_events: Vec<SecurityEventSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEventSummary {
    pub occurred_at: String,
    pub event_type: String,
    pub severity: String,
    pub detail: Option<String>,
}
