//! Background daemon (systemd user service, running in the *child's own*
//! account — see docs/agent-protocol.md for why, not the separate parent
//! account). Owns time-budget enforcement, the app-wrapper event socket, the
//! PIN/polkit override path, and the SQLite usage log.

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
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};

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

    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                eprintln!("omarchy-kids-agentd: accept failed: {e}");
                continue;
            }
        };
        let state = Arc::clone(&state);
        std::thread::spawn(move || handle_connection(stream, &state));
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
    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };
    let mut writer = stream;

    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return; // client disconnected without sending anything
    }

    let response = match serde_json::from_str::<Request>(line.trim_end()) {
        Ok(req) => handlers::dispatch(state, req),
        Err(e) => Response::err(format!("malformed request: {e}")),
    };

    if let Ok(text) = serde_json::to_string(&response) {
        let _ = writeln!(writer, "{text}");
    }
}
