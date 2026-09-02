//! `omarchy-kids-run <desktop-id>` — every unlocked app is launched through
//! this wrapper instead of directly, so agentd can see start/stop and (via
//! periodic polling) cut a session short when a time budget or window is
//! exhausted. See `docs/agent-protocol.md` and issue #5.

use omarchy_kids_common::desktop;
use omarchy_kids_common::paths;
use omarchy_kids_common::protocol::Request;
use omarchy_kids_common::transport;
use std::process::{Child, Command, ExitCode};
use std::time::{Duration, Instant};

const AGENTD_TIMEOUT: Duration = Duration::from_millis(800);
const POLL_INTERVAL: Duration = Duration::from_secs(15);
const KILL_GRACE: Duration = Duration::from_secs(5);

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(desktop_id) = args.next() else {
        eprintln!("usage: omarchy-kids-run <desktop-id>");
        return ExitCode::from(2);
    };

    let socket = paths::socket_path();
    // Authorize *before* ever resolving/spawning anything (issue #35) — the
    // previous design spawned first and only found out via the first poll,
    // up to POLL_INTERVAL later, that the app wasn't allowed. Unlike the
    // fail-open WrapperStart/Stop/Poll notifications below, this check
    // fails closed: an unreachable agentd means nothing launches, not
    // "anything goes" — see the doc comment on `Request::WrapperAuthorize`.
    match authorize(&socket, &desktop_id) {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("omarchy-kids-run: '{desktop_id}' is not authorized to launch");
            return ExitCode::FAILURE;
        }
        Err(err) => {
            eprintln!("omarchy-kids-run: refusing to launch '{desktop_id}': {err}");
            return ExitCode::FAILURE;
        }
    }

    let Some(desktop_file) = desktop::find_desktop_file(&desktop_id) else {
        eprintln!("omarchy-kids-run: no .desktop entry found for '{desktop_id}'");
        return ExitCode::FAILURE;
    };
    let entry = match desktop::parse_desktop_entry(&desktop_file) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("omarchy-kids-run: {err:#}");
            return ExitCode::FAILURE;
        }
    };
    let argv = desktop::exec_to_argv(&entry.exec);
    let Some((program, rest)) = argv.split_first() else {
        eprintln!(
            "omarchy-kids-run: empty Exec= in {}",
            desktop_file.display()
        );
        return ExitCode::FAILURE;
    };

    let mut child = match Command::new(program).args(rest).spawn() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("omarchy-kids-run: failed to launch '{program}': {err}");
            return ExitCode::FAILURE;
        }
    };
    let pid = child.id();

    report(
        &socket,
        &Request::WrapperStart {
            app: desktop_id.clone(),
            pid,
        },
    );

    let mut last_poll = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {}
            Err(err) => {
                eprintln!("omarchy-kids-run: wait() failed: {err}");
                break;
            }
        }

        if last_poll.elapsed() >= POLL_INTERVAL {
            last_poll = Instant::now();
            if !still_allowed(&socket, &desktop_id, pid) {
                terminate(&mut child, pid);
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    let exit_code = child.try_wait().ok().flatten().and_then(|s| s.code());
    report(
        &socket,
        &Request::WrapperStop {
            app: desktop_id,
            pid,
            exit_code,
        },
    );

    ExitCode::SUCCESS
}

/// Fail-closed pre-launch check (issue #35) — the opposite policy from
/// `report`/`still_allowed` below on purpose: those cover an
/// already-running, already-authorized session (an agentd outage shouldn't
/// kill it), whereas this gates whether anything launches at all.
fn authorize(socket: &std::path::Path, app: &str) -> Result<bool, String> {
    let req = Request::WrapperAuthorize {
        app: app.to_string(),
    };
    match transport::send(socket, &req, AGENTD_TIMEOUT) {
        Ok(resp) if resp.ok => Ok(resp
            .data
            .and_then(|d| d.get("allowed").and_then(|v| v.as_bool()))
            .unwrap_or(false)),
        Ok(resp) => Err(resp
            .error
            .unwrap_or_else(|| "agentd rejected the authorization check".to_string())),
        Err(err) => Err(format!("could not reach agentd ({err})")),
    }
}

/// Fail-open (issue #6): if agentd is unreachable, the app keeps running
/// rather than the child being stranded on a computer that silently does
/// nothing. The kiosk's actual access-control boundary is the VT/getty
/// lockdown (see CLAUDE.md), not this wrapper — an agentd outage degrades
/// time-budget enforcement, it doesn't need to take down the session.
/// Failures are logged (journald captures the wrapper's stderr) but never
/// block or kill the app.
fn report(socket: &std::path::Path, req: &Request) {
    if let Err(err) = transport::send(socket, req, AGENTD_TIMEOUT) {
        eprintln!("omarchy-kids-run: could not reach agentd ({err}); continuing (fail-open)");
    }
}

fn still_allowed(socket: &std::path::Path, app: &str, pid: u32) -> bool {
    let req = Request::WrapperPoll {
        app: app.to_string(),
        pid,
    };
    match transport::send(socket, &req, AGENTD_TIMEOUT) {
        Ok(resp) if resp.ok => resp
            .data
            .and_then(|d| d.get("allowed").and_then(|v| v.as_bool()))
            .unwrap_or(true),
        Ok(resp) => {
            eprintln!("omarchy-kids-run: agentd rejected poll: {:?}", resp.error);
            true
        }
        Err(err) => {
            eprintln!("omarchy-kids-run: poll failed ({err}); continuing (fail-open)");
            true
        }
    }
}

fn terminate(child: &mut Child, pid: u32) {
    eprintln!("omarchy-kids-run: agentd says pid {pid} is no longer allowed, terminating");
    // SAFETY: pid is our own direct child's pid, obtained from `Child::id()`
    // just above — signalling it is safe.
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    let deadline = Instant::now() + KILL_GRACE;
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = child.kill(); // SIGKILL if it ignored SIGTERM
    let _ = child.wait();
}
