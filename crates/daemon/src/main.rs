use anyhow::Result;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod health;
mod lifecycle;
mod scheduler;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("shiftwrangler daemon starting");

    // Config, agents, platform, and state backend are wired up here.
    // The CLI crate is the primary entry point; this binary is for running
    // the daemon directly (e.g. as a systemd service).

    tokio::signal::ctrl_c().await?;
    info!("shutting down");
    Ok(())
}
