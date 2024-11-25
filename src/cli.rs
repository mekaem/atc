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
    /// Initialize a new Bluesky configuration
    Init(InitArgs),

    /// Start Bluesky services
    Start(StartArgs),

    /// Stop Bluesky services
    Stop(StopArgs),

    /// Create a new account
    CreateAccount(CreateAccountArgs),

    /// Deploy feed generator service
    DeployFeed(DeployFeedArgs),

    /// Show service status
    Status(StatusArgs),

    /// Check environment readiness
    Check(CheckArgs),

    /// Manage certificates
    Certs(CertArgs),

    /// Deploy Ozone (moderation) service
    DeployOzone(DeployOzoneArgs),

    /// Configure Ozone admin
    ConfigureOzone(ConfigureOzoneArgs),

    /// Check service health
    Health(HealthArgs),

    /// Deploy and configure Jetstream
    DeployJetstream(DeployJetstreamArgs),

    /// Subscribe to Jetstream
    Subscribe(SubscribeArgs),
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Domain name for the Bluesky instance
    #[arg(long)]
    pub domain: String,

    /// Email for Let's Encrypt certificates
    #[arg(long)]
    pub cert_email: String,
}

#[derive(Args, Debug)]
pub struct StartArgs {
    /// Specific services to start (all if not specified)
    #[arg(long)]
    pub services: Option<Vec<String>>,

    /// Skip dependency checks
    #[arg(long)]
    pub no_deps: bool,
}

#[derive(Args, Debug)]
pub struct StopArgs {
    /// Remove containers and volumes
    #[arg(long)]
    pub clean: bool,
}

#[derive(Args, Debug)]
pub struct CreateAccountArgs {
    /// Account handle (e.g., user.domain.com)
    pub handle: String,
    /// Account email
    pub email: String,
    /// Account password
    pub password: String,
}

#[derive(Args, Debug)]
pub struct DeployFeedArgs {
    /// Publisher DID
    #[arg(long)]
    pub publisher_did: String,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Show detailed information
    #[arg(long)]
    pub verbose: bool,
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

#[derive(Args, Debug)]
pub struct CertArgs {
    /// Generate and install self-signed certificates
    #[arg(long)]
    pub self_signed: bool,
}

#[derive(Args, Debug)]
pub struct DeployOzoneArgs {
    /// Server DID
    #[arg(long)]
    pub server_did: String,

    /// Admin DIDs (comma-separated)
    #[arg(long)]
    pub admin_dids: String,
}

#[derive(Args, Debug)]
pub struct ConfigureOzoneArgs {
    /// Admin handle
    #[arg(long)]
    pub handle: String,

    /// PLC sign token
    #[arg(long)]
    pub plc_sign_token: String,

    /// Ozone URL
    #[arg(long)]
    pub ozone_url: Option<String>,
}

#[derive(Args, Debug)]
pub struct HealthArgs {
    /// Specific services to check (all if not specified)
    #[arg(long)]
    pub services: Option<Vec<String>>,

    /// Include detailed health metrics
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct DeployJetstreamArgs {
    /// Custom reconnect delay in milliseconds
    #[arg(long)]
    pub reconnect_delay: Option<u32>,
}

#[derive(Args, Debug)]
pub struct SubscribeArgs {
    /// Collections to subscribe to (comma-separated)
    #[arg(long)]
    pub collections: Vec<String>,
}
