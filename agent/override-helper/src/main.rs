//! `omarchy-kids-override-helper` — the actual target of `pkexec` for the
//! PIN/polkit override path (issue #9). Never invoked directly: `pkexec`
//! authenticates the caller against the `net.omarchykids.agent.override-unlock`
//! polkit action (see `packaging/polkit/`, which requires `auth_admin` — i.e.
//! the separate parent/admin account, not the child's own account) and only
//! then runs this binary as the child's own user.
//!
//! `pkexec` sanitizes the environment before exec, so `HOME`/`XDG_RUNTIME_DIR`
//! don't survive — the caller (`omarchy-kids-agent override`) passes them
//! explicitly instead of relying on inheritance.

use clap::Parser;
use omarchy_kids_common::paths;
use omarchy_kids_common::protocol::Request;
use omarchy_kids_common::transport;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Parser)]
struct Cli {
    app: String,
    #[arg(long)]
    minutes: Option<u32>,
    #[arg(long)]
    home: String,
    #[arg(long)]
    runtime_dir: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    std::env::set_var("HOME", &cli.home);
    std::env::set_var("XDG_RUNTIME_DIR", &cli.runtime_dir);

    let socket = paths::socket_path();
    let req = Request::Override {
        app: cli.app,
        minutes: cli.minutes,
    };

    match transport::send(&socket, &req, Duration::from_secs(3)) {
        Ok(resp) if resp.ok => {
            println!("unlocked");
            ExitCode::SUCCESS
        }
        Ok(resp) => {
            eprintln!(
                "omarchy-kids-override-helper: agentd refused: {}",
                resp.error.unwrap_or_default()
            );
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("omarchy-kids-override-helper: could not reach agentd: {err}");
            ExitCode::FAILURE
        }
    }
}
