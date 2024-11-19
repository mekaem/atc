use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Bluesky self-hosting manager
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Check environment readiness
    Check(CheckArgs),
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Skip DNS checks
    #[arg(long)]
    pub no_dns: bool,

    /// Skip Docker checks
    #[arg(long)]
    pub no_docker: bool,
}
