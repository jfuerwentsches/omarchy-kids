//! Security-relevant event delivery (issue #10). Routine events are only
//! ever surfaced passively (via `agent report`, read by the Control Center);
//! severe events additionally get an active local notification here.
//!
//! Cross-host delivery to the *parent's* computer — the other half of the
//! design note's "gestufte Zustellung" — depends on the Control Center,
//! which is still a stub (see CLAUDE.md "Status": `control/` has no real
//! logic yet). Until that exists there's nothing on the other end to push
//! to, so this deliberately stops at local delivery + the usage log, which
//! is what the Control Center will read once it can poll `agent report`.

use std::process::Command;

pub fn notify_severe(message: &str) {
    let result = Command::new("notify-send")
        .args(["--urgency=critical", "Omarchy Kids", message])
        .status();
    if let Err(e) = result {
        eprintln!(
            "agentd: notify-send failed ({e}); severe event is still recorded in the usage db"
        );
    }
}
