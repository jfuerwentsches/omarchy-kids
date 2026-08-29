//! Thin CLI reached over SSH (`command=`-restricted key — see
//! `packaging/`). Forwards commands to `omarchy-kids-agentd` over the local
//! socket and formats the response (human-readable and/or JSON for the
//! Control Center). Never touches the live desktop session directly — see
//! CLAUDE.md's "Architektur-Begründung" for why that split matters.

use clap::{Parser, Subcommand};
use omarchy_kids_common::paths;
use omarchy_kids_common::protocol::{AppUsage, Request, Response, ReportPayload, StatusPayload};
use omarchy_kids_common::transport;
use std::path::{Path, PathBuf};
use std::process::{Command as OsCommand, ExitCode};
use std::time::Duration;

const AGENTD_TIMEOUT: Duration = Duration::from_secs(3);
/// Installed by the package's polkit action
/// (`packaging/polkit/net.omarchykids.agent.policy`), which is what makes
/// `pkexec` prompt for the parent/admin account rather than just running.
const OVERRIDE_HELPER_PATH: &str = "/usr/lib/omarchy-kids/omarchy-kids-override-helper";
/// Same idea as `OVERRIDE_HELPER_PATH`, for the pairing re-trigger path
/// (issue #25's remaining gap — see `run_repair_pairing`).
const REPAIR_HELPER_PATH: &str = "/usr/lib/omarchy-kids/omarchy-kids-repair-helper";
/// Matches `omarchy-kids-pairing serve`'s own CLI defaults and the setup
/// wizard's `PAIRING_PORT`/`PAIRING_TIMEOUT_MINUTES` (see
/// `setup-wizard/first-boot/omarchy-kids-setup-wizard`) — nothing new for a
/// future reader to reconcile between the three.
const DEFAULT_PAIRING_PORT: u16 = 7420;
const DEFAULT_PAIRING_TIMEOUT_MINUTES: u32 = 10;

#[derive(Parser)]
#[command(name = "omarchy-kids-agent")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Show current tier, unlocked apps, and remaining time budget.
    Status,
    /// Temporarily unlock an app outside the tier's fixed app list.
    Unlock {
        app: String,
        #[arg(long)]
        minutes: Option<u32>,
    },
    /// Same as `unlock`, but authenticated locally via polkit (`pkexec`) —
    /// the "PIN entry at the child's computer" emergency path (issue #9).
    /// Run this on the child computer itself, not over SSH.
    Override {
        app: String,
        #[arg(long)]
        minutes: Option<u32>,
    },
    /// Print a usage report.
    Report {
        #[arg(long)]
        week: bool,
    },
    /// Switch the active age tier. `--apps-file` (as passed by
    /// `omarchy-kids-set-tier`) replaces agentd's record of the tier's base
    /// tile list — see the `SetTier` doc comment in `omarchy_kids_common::protocol`.
    SetTier {
        tier: String,
        #[arg(long)]
        apps_file: Option<PathBuf>,
    },
    /// Re-open the pairing window (mDNS + QR, same as first boot) so the
    /// Control Center can pair with this machine again, authenticated
    /// locally via polkit (issue #25). Run this on the child computer
    /// itself, not over SSH — same "PIN entry at the child's computer"
    /// pattern as `override`.
    RepairPairing {
        #[arg(long, default_value_t = DEFAULT_PAIRING_PORT)]
        port: u16,
        #[arg(long, default_value_t = DEFAULT_PAIRING_TIMEOUT_MINUTES)]
        timeout_minutes: u32,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse_from(effective_args()) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    };
    let socket = paths::socket_path();

    let result = match &cli.command {
        Command::Override { app, minutes } => run_override(&socket, app, *minutes),
        Command::SetTier { tier, apps_file } => {
            run_set_tier(&socket, tier.clone(), apps_file.clone(), cli.json)
        }
        Command::RepairPairing {
            port,
            timeout_minutes,
        } => run_repair_pairing(*port, *timeout_minutes),
        _ => {
            let req = to_request(&cli.command);
            send_and_print(&socket, req, cli.json)
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("omarchy-kids-agent: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// The `command=` restriction in the pairing-installed `authorized_keys`
/// entry (see `agent/pairing/src/main.rs`'s `install_pubkey`) always execs
/// this binary with **zero** argv — sshd never forwards what the client
/// actually asked for (`ssh host status --json`) as arguments; it only
/// stashes it in `$SSH_ORIGINAL_COMMAND`. Without reading that back out,
/// every remote invocation silently collapsed to a bare `omarchy-kids-agent`
/// (clap's "no subcommand given" usage error, exit 2) — the Control
/// Center's SSH-based polling never actually reached agentd. Only consulted
/// when the process's own argv is otherwise empty (just argv[0]), so a local
/// invocation (`override`, `repair-pairing`, or a real interactive-shell SSH
/// session with no forced command) is never affected — `$SSH_ORIGINAL_COMMAND`
/// is only set by sshd in exactly the forced-command case this exists for.
fn effective_args() -> Vec<String> {
    let mut argv: Vec<String> = std::env::args().collect();
    if argv.len() > 1 {
        return argv;
    }
    if let Ok(original) = std::env::var("SSH_ORIGINAL_COMMAND") {
        if let Some(mut parsed) = shlex::split(&original) {
            argv.append(&mut parsed);
        }
    }
    argv
}

fn to_request(cmd: &Command) -> Request {
    match cmd {
        Command::Status => Request::Status,
        Command::Unlock { app, minutes } => Request::Unlock {
            app: app.clone(),
            minutes: *minutes,
        },
        Command::Report { week } => Request::Report { week: *week },
        Command::SetTier { .. } => unreachable!("handled by run_set_tier"),
        Command::Override { .. } => unreachable!("handled by run_override"),
        Command::RepairPairing { .. } => unreachable!("handled by run_repair_pairing"),
    }
}

fn send_and_print(socket: &Path, req: Request, json_out: bool) -> Result<(), String> {
    let resp = transport::send(socket, &req, AGENTD_TIMEOUT).map_err(|e| e.to_string())?;

    if json_out {
        let text = serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())?;
        println!("{text}");
        return if resp.ok {
            Ok(())
        } else {
            Err(resp.error.unwrap_or_default())
        };
    }

    if !resp.ok {
        return Err(resp.error.unwrap_or_else(|| "agentd returned an error".into()));
    }

    match &req {
        Request::Status => print_status(&resp)?,
        Request::Report { .. } => print_report(&resp)?,
        Request::Unlock { app, minutes } => println!(
            "Unlocked '{app}' for {} minutes.",
            minutes.unwrap_or(omarchy_kids_common::protocol::DEFAULT_UNLOCK_MINUTES)
        ),
        Request::SetTier { tier, .. } => println!("Tier set to '{tier}'."),
        _ => {}
    }
    Ok(())
}

fn print_status(resp: &Response) -> Result<(), String> {
    let payload: StatusPayload = serde_json::from_value(resp.data.clone().unwrap_or_default())
        .map_err(|e| format!("malformed status payload: {e}"))?;
    println!("tier:              {}", payload.tier);
    println!("unlocked apps:     {}", payload.unlocked_apps.join(", "));
    println!(
        "daily budget:      {} / {} min used",
        payload.daily_used_minutes, payload.daily_budget_minutes
    );
    println!("remaining today:   {} min", payload.daily_remaining_minutes);
    println!("blocked window:    {}", payload.in_blocked_window);
    println!(
        "active app:        {}",
        payload.active_app.as_deref().unwrap_or("(none)")
    );
    Ok(())
}

fn print_report(resp: &Response) -> Result<(), String> {
    let payload: ReportPayload = serde_json::from_value(resp.data.clone().unwrap_or_default())
        .map_err(|e| format!("malformed report payload: {e}"))?;
    println!("period:            last {} day(s)", payload.range_days);
    println!(
        "total this period: {} min (previous: {} min)",
        payload.total_minutes_this_period, payload.total_minutes_previous_period
    );
    println!("pin overrides:     {}", payload.pin_override_count);
    println!();
    println!("{:<30} {:>10}", "app", "minutes");
    for AppUsage { app, minutes } in &payload.per_app {
        println!("{app:<30} {minutes:>10}");
    }
    if !payload.security_events.is_empty() {
        println!();
        println!("security events:");
        for ev in &payload.security_events {
            println!(
                "  [{}] {} ({}){}",
                ev.occurred_at,
                ev.event_type,
                ev.severity,
                ev.detail
                    .as_ref()
                    .map(|d| format!(" — {d}"))
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}

fn run_set_tier(
    socket: &Path,
    tier: String,
    apps_file: Option<PathBuf>,
    json_out: bool,
) -> Result<(), String> {
    let apps = match apps_file {
        Some(path) => {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            let apps = serde_json::from_str(&raw)
                .map_err(|e| format!("parsing {}: {e}", path.display()))?;
            Some(apps)
        }
        None => None,
    };
    send_and_print(socket, Request::SetTier { tier, apps }, json_out)
}

/// Runs the override-helper under `pkexec`, which is what actually gates this
/// on polkit authentication as the parent/admin account (see
/// `packaging/polkit/net.omarchykids.agent.policy`) — this process just
/// shells out and reports the outcome, it never touches agentd's socket
/// directly for the unlock itself.
fn run_override(socket: &Path, app: &str, minutes: Option<u32>) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let runtime_dir =
        std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR is not set".to_string())?;
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .map_err(|_| "cannot determine the current user (USER/LOGNAME unset)".to_string())?;

    let mut cmd = OsCommand::new("pkexec");
    cmd.arg("--user")
        .arg(&username)
        .arg(OVERRIDE_HELPER_PATH)
        .arg(app)
        .arg("--home")
        .arg(&home)
        .arg("--runtime-dir")
        .arg(&runtime_dir);
    if let Some(m) = minutes {
        cmd.arg("--minutes").arg(m.to_string());
    }

    let status = cmd
        .status()
        .map_err(|e| format!("failed to run pkexec: {e}"))?;

    if status.success() {
        println!("Unlocked '{app}' via override.");
        Ok(())
    } else {
        // pkexec: 126 = not authorized, 127 = auth dialog failed/dismissed.
        // Either way agentd never heard about this attempt on its own —
        // tell it explicitly so repeated failures can be flagged (issue #10).
        let _ = transport::send(
            socket,
            &Request::OverrideFailed {
                app: app.to_string(),
            },
            AGENTD_TIMEOUT,
        );
        Err(format!(
            "override authentication failed or was cancelled (pkexec exit {:?})",
            status.code()
        ))
    }
}

/// Runs the repair-helper under `pkexec` (no `--user`, unlike `run_override`
/// — the helper needs to run as root to toggle the pairing UFW rule, then
/// drops to this account itself via `sudo -u` before invoking
/// `omarchy-kids-pairing serve`; see `repair-helper/src/main.rs`). Fixes
/// issue #25's one remaining gap: there was previously no way to re-trigger
/// pairing on a production machine, since the child account has no
/// reachable shell and the paired SSH channel's `command=` restriction only
/// ever runs this same binary's non-pairing subcommands.
fn run_repair_pairing(port: u16, timeout_minutes: u32) -> Result<(), String> {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .map_err(|_| "cannot determine the current user (USER/LOGNAME unset)".to_string())?;

    let status = OsCommand::new("pkexec")
        .arg(REPAIR_HELPER_PATH)
        .arg("--child-user")
        .arg(&username)
        .arg("--port")
        .arg(port.to_string())
        .arg("--timeout-minutes")
        .arg(timeout_minutes.to_string())
        .status()
        .map_err(|e| format!("failed to run pkexec: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        // pkexec: 126 = not authorized, 127 = auth dialog failed/dismissed.
        // Anything else means the helper ran but `omarchy-kids-pairing
        // serve` itself was skipped/timed out/failed — same best-effort
        // semantics as the first-boot wizard's own run_pairing, so this is
        // reported but not treated as a security event (unlike a failed
        // override, this isn't gated on secret knowledge an attacker could
        // brute-force).
        Err(format!(
            "pairing window closed without a successful pairing (pkexec exit {:?})",
            status.code()
        ))
    }
}
