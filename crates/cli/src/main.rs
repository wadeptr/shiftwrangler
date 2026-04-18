mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "swctl",
    about = "Session lifecycle manager — pause, sleep, wake, resume",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the background daemon.
    Daemon {
        #[command(subcommand)]
        cmd: commands::daemon::DaemonCommand,
    },
    /// Manage tracked agent sessions.
    Session {
        #[command(subcommand)]
        cmd: commands::session::SessionCommand,
    },
    /// Configure suspend/wake schedule.
    Schedule {
        #[command(subcommand)]
        cmd: commands::schedule::ScheduleCommand,
    },
    /// Manually trigger suspend or resume.
    Suspend {
        #[command(subcommand)]
        cmd: commands::suspend::SuspendCommand,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Daemon { cmd } => commands::daemon::handle(cmd).await,
        Command::Session { cmd } => commands::session::handle(cmd).await,
        Command::Schedule { cmd } => commands::schedule::handle(cmd).await,
        Command::Suspend { cmd } => commands::suspend::handle(cmd).await,
    }
}
