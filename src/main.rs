mod api;
mod caddy;
mod certs;
mod cli;
mod commands;
mod compose;
mod config;
mod dns;
mod docker;
mod error;
mod feed;
mod health;
mod jetstream;
mod ozone;
mod secrets;
mod status;

use clap::Parser;
use cli::Cli;
use error::Result;
use owo_colors::OwoColorize;
use tracing::error;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    if let Err(e) = commands::handle_command(cli.command, &cli.config).await {
        error!("{}", e);
        eprintln!("{}", format!("Error: {}", e).red().bold());
        std::process::exit(1);
    }

    Ok(())
}
