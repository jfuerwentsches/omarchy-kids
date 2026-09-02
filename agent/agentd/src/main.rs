//! Background daemon (systemd user service, running in the *child's own*
//! account — see docs/agent-protocol.md for why, not the separate parent
//! account). Owns time-budget enforcement, the app-wrapper event socket, the
//! PIN/polkit override path, and the SQLite usage log.

mod allowlist;
mod budget;
mod db;
mod handlers;
mod launcher;
mod prewarning;
mod security;
mod state;
mod ticker;

use anyhow::{Context, Result};
use omarchy_kids_common::config::Config;
use omarchy_kids_common::paths;
use omarchy_kids_common::protocol::{Request, Response};
use omarchy_kids_common::transport::{read_line_bounded, MAX_LINE_BYTES};
use std::io::{BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Caps how many connection-handling threads can be alive at once (issue
/// #36) — each is short-lived (one request/response, then it exits), so this
/// only guards against a flood of slow/idle connections pinning down
/// unbounded threads; it isn't a normal-usage limit.
const MAX_CONCURRENT_CONNECTIONS: usize = 32;
/// A connection that neither sends its request line nor reads its response
/// within this long is dropped (issue #36) — real requests are one
/// round-trip of small JSON, answered well within a second.
const CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

use state::State;

fn main() -> Result<()> {
    let config_path = paths::config_path();
    let default_tier =
        std::env::var("OMARCHY_KIDS_DEFAULT_TIER").unwrap_or_else(|_| "mini".to_string());
    let config = Config::load_or_init(&config_path, &default_tier).context("loading config")?;

    let db = db::open(&paths::db_path()).context("opening usage database")?;

    launcher::render(&config).context("rendering launcher-apps.json")?;

    let state = Arc::new(Mutex::new(State {
        config,
        config_path,
        db,
        active: Default::default(),
    }));

    let socket_path = paths::socket_path();
    let listener = bind_socket(&socket_path)?;
    eprintln!("omarchy-kids-agentd: listening on {}", socket_path.display());

    ticker::spawn(Arc::clone(&state));

    let active_connections = Arc::new(AtomicUsize::new(0));

    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                eprintln!("omarchy-kids-agentd: accept failed: {e}");
                continue;
            }
        };
        if active_connections.load(Ordering::Relaxed) >= MAX_CONCURRENT_CONNECTIONS {
            eprintln!(
                "omarchy-kids-agentd: at the concurrent-connection limit ({MAX_CONCURRENT_CONNECTIONS}), dropping a connection"
            );
            drop(stream);
            continue;
        }
        active_connections.fetch_add(1, Ordering::Relaxed);
        let state = Arc::clone(&state);
        let active_connections = Arc::clone(&active_connections);
        std::thread::spawn(move || {
            handle_connection(stream, &state);
            active_connections.fetch_sub(1, Ordering::Relaxed);
        });
    }
    Ok(())
}

fn bind_socket(path: &Path) -> Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("chmod {}", parent.display()))?;
    }
    if path.exists() {
        // A stale socket from a previous run that didn't shut down cleanly
        // (e.g. killed rather than stopped) — remove and rebind rather than
        // fail to start.
        std::fs::remove_file(path)
            .with_context(|| format!("removing stale socket {}", path.display()))?;
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("binding {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("chmod {}", path.display()))?;
    Ok(listener)
}

fn handle_connection(stream: UnixStream, state: &Arc<Mutex<State>>) {
    if stream.set_read_timeout(Some(CONNECTION_IDLE_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(CONNECTION_IDLE_TIMEOUT)).is_err()
    {
        return;
    }

    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };
    let mut writer = stream;

    let line = match read_line_bounded(&mut reader, MAX_LINE_BYTES) {
        Ok(Some(line)) => line,
        Ok(None) => return, // client disconnected without sending anything
        Err(e) => {
            eprintln!("omarchy-kids-agentd: rejecting connection: {e}");
            return;
        }
    };

    let response = match serde_json::from_str::<Request>(&line) {
        Ok(req) => handlers::dispatch(state, req),
        Err(e) => Response::err(format!("malformed request: {e}")),
    };

    if let Ok(text) = serde_json::to_string(&response) {
        let _ = writeln!(writer, "{text}");
    }
}
