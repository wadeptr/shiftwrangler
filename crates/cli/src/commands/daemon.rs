use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DaemonCommand {
    /// Start the background daemon.
    Start,
    /// Stop the background daemon.
    Stop,
    /// Show daemon status.
    Status,
}

pub async fn handle(cmd: DaemonCommand) -> Result<()> {
    match cmd {
        DaemonCommand::Start => {
            println!("Starting shiftwrangler daemon...");
            // TODO: spawn daemon process / systemd unit
        }
        DaemonCommand::Stop => {
            println!("Stopping shiftwrangler daemon...");
        }
        DaemonCommand::Status => {
            println!("Daemon status: not yet implemented");
        }
    }
    Ok(())
}
