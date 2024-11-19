use crate::{
    cli::Commands,
    config::Config,
    dns::DnsChecker,
    docker::DockerService,
    error::{Error, Result},
};
use owo_colors::OwoColorize;
use std::{fs, path::Path};
use tracing::{info, warn};

pub async fn handle_command(cmd: Commands, config_path: &Path) -> Result<()> {
    match cmd {
        Commands::Check(args) => {
            let config = Config::load(config_path)?;
            info!("Checking environment readiness...");

            // Check required files exist
            if !Path::new("docker-compose.yml").exists() {
                return Err(Error::Config(
                    "docker-compose.yml not found. Run init first.".into(),
                ));
            }

            if !Path::new("config/caddy").exists() {
                return Err(Error::Config("config/caddy directory not found".into()));
            }

            if !args.no_dns {
                info!("Checking DNS configuration...");
                if DnsChecker::check_domain(&config.network.domain).await? {
                    println!("{}", "DNS configuration: OK".green());
                } else {
                    return Err(Error::Network("DNS checks failed".into()));
                }

                info!("Testing HTTPS endpoint...");
                if DnsChecker::check_ssl_test_endpoint(&config.network.domain).await? {
                    println!("{}", "HTTPS endpoint: OK".green());
                } else {
                    return Err(Error::Network("HTTPS endpoint test failed".into()));
                }

                info!("Testing WebSocket endpoint...");
                if DnsChecker::check_websocket_endpoint(&config.network.domain).await? {
                    println!("{}", "WebSocket endpoint: OK".green());
                } else {
                    return Err(Error::Network("WebSocket endpoint test failed".into()));
                }
            }

            if !args.no_docker {
                info!("Checking Docker dependencies...");
                DockerService::check_dependencies().await?;
                println!("{}", "Docker dependencies: OK".green());
            }

            println!("{}", "Environment check completed successfully!".green());
            Ok(())
        }
    }
}
