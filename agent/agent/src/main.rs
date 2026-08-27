use clap::{Parser, Subcommand};

/// Thin CLI reached over SSH (command= restricted key). Forwards to omarchy-kids-agentd
/// over the local socket and formats the response (human-readable and/or JSON).
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
    /// Temporarily unlock an app.
    Unlock { app: String },
    /// Print a usage report.
    Report {
        #[arg(long)]
        week: bool,
    },
    /// Switch the active age tier.
    SetTier { tier: String },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Status => todo!("query omarchy-kids-agentd over the local socket"),
        Command::Unlock { app } => todo!("forward unlock({app}) to omarchy-kids-agentd"),
        Command::Report { week } => todo!("forward report(week={week}) to omarchy-kids-agentd"),
        Command::SetTier { tier } => todo!("forward set-tier({tier}) to omarchy-kids-agentd"),
    }
}
