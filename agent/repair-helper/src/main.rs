//! `omarchy-kids-repair-helper` — the `pkexec` target for re-triggering
//! pairing on a production machine (issue #25's one remaining gap: the child
//! account has no reachable shell, and the paired SSH channel's `command=`
//! restriction only ever runs `omarchy-kids-agent`, so there's no way for a
//! parent to re-run `omarchy-kids-pairing serve` on a machine that skipped or
//! lost pairing). Never invoked directly: `pkexec` authenticates the caller
//! against the `net.omarchykids.agent.repair-pairing` polkit action (see
//! `packaging/polkit/`, `auth_admin` — the separate parent/admin account) and
//! only then runs this binary, as root (unlike `omarchy-kids-override-helper`,
//! which drops back to the child account via `--user`).
//!
//! Root is needed here for the same reason the setup wizard's
//! `open_pairing_ufw_rule`/`close_pairing_ufw_rule` (see
//! `setup-wizard/first-boot/omarchy-kids-setup-wizard`) run as root: only root
//! can toggle the pairing port's UFW rule. This helper mirrors that script's
//! `run_pairing` almost exactly — open the rule, run `omarchy-kids-pairing
//! serve` as the child account via `sudo -u`, always close the rule after —
//! just reached via polkit instead of the first-boot unit.

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::process::{Command as OsCommand, ExitCode};

#[derive(Parser)]
struct Cli {
    /// The child account to run `omarchy-kids-pairing serve` as — the same
    /// account `omarchy-kids-agent repair-pairing` was invoked from.
    #[arg(long)]
    child_user: String,
    #[arg(long, default_value_t = 7420)]
    port: u16,
    #[arg(long, default_value_t = 10)]
    timeout_minutes: u32,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if !is_root() {
        eprintln!("omarchy-kids-repair-helper: must run as root (expected to be reached via pkexec)");
        return ExitCode::FAILURE;
    }

    match run(&cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("omarchy-kids-repair-helper: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn is_root() -> bool {
    // SAFETY: getuid() has no preconditions and never fails.
    unsafe { libc::getuid() == 0 }
}

/// Returns whether `omarchy-kids-pairing serve` itself succeeded. The UFW
/// rule is always closed on the way out, success or not — same "scoped to
/// exactly the pairing window" reasoning as the wizard's own comment.
fn run(cli: &Cli) -> Result<bool> {
    open_pairing_ufw_rule(cli.port).context("opening the pairing UFW rule")?;

    let status = OsCommand::new("sudo")
        .arg("-u")
        .arg(&cli.child_user)
        .arg("omarchy-kids-pairing")
        .arg("serve")
        .arg("--port")
        .arg(cli.port.to_string())
        .arg("--timeout-minutes")
        .arg(cli.timeout_minutes.to_string())
        .status();

    close_pairing_ufw_rule(cli.port);

    let status = status.context("running omarchy-kids-pairing serve via sudo")?;
    Ok(status.success())
}

fn ufw_active() -> bool {
    let Ok(output) = OsCommand::new("ufw").arg("status").output() else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout).contains("Status: active")
}

fn open_pairing_ufw_rule(port: u16) -> Result<()> {
    if !ufw_active() {
        return Ok(());
    }
    let status = OsCommand::new("ufw")
        .args(["allow", &format!("{port}/tcp"), "comment", "omarchy-kids pairing window"])
        .status()
        .context("running ufw allow")?;
    if !status.success() {
        bail!("ufw allow exited with {status}");
    }
    Ok(())
}

fn close_pairing_ufw_rule(port: u16) {
    if !ufw_active() {
        return;
    }
    // Best-effort, matching the wizard's own close_pairing_ufw_rule: a failed
    // delete shouldn't mask whether pairing itself succeeded.
    let _ = OsCommand::new("ufw")
        .args(["--force", "delete", "allow", &format!("{port}/tcp")])
        .status();
}
