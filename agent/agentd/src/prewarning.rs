//! Pre-warning delivery via Quickshell IPC (issue #8). Confirmed against the
//! Quickshell instance running on the dev VM: Omarchy's own `omarchy-shell
//! <target> <method> [args...]` wrapper is the canonical way to reach a
//! running shell's `IpcHandler` — it already handles instance discovery
//! (`qs ipc -n -p "$OMARCHY_PATH/shell" call ...`) and, notably, recovering
//! `WAYLAND_DISPLAY` for callers outside the graphical session's own env
//! (relevant since agentd is a systemd --user service, not a session client
//! itself). Reusing it instead of hand-rolling a `qs ipc call` avoids
//! duplicating that fragile bit.
//!
//! The launcher plugin registers `IpcHandler { target: "omarchyKidsLauncher"
//! }` with a `preWarn(app, secondsLeft)` function (see Launcher.qml) — mini
//! tier's non-readers get an acoustic cue there, per design note "Omarchy
//! Kids - Parental Controls und Bildschirmzeit".

use std::process::Command;

const IPC_TARGET: &str = "omarchyKidsLauncher";

/// Fire-and-forget: a missing/unresponsive shell (or no Quickshell instance
/// running at all, e.g. during headless testing) must never block or crash
/// agentd's enforcement loop — `omarchy-shell -q` already treats all of that
/// as a quiet no-op (exit 0).
pub fn trigger(app: &str, minutes_left: u32) {
    let seconds_left = (minutes_left * 60).to_string();
    let result = Command::new("omarchy-shell")
        .args(["-q", IPC_TARGET, "preWarn", app, &seconds_left])
        .status();
    if let Err(e) = result {
        eprintln!("agentd: pre-warning IPC call failed to launch: {e}");
    }
}
