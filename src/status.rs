use crate::error::Result;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;
use tracing::{debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub running: bool,
    pub healthy: bool,
    pub endpoint: Option<String>,
    pub version: Option<String>,
    pub details: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub services: HashMap<String, ServiceStatus>,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
}

pub struct StatusManager {
    docker: crate::docker::DockerService,
}

impl StatusManager {
    pub fn new(docker: crate::docker::DockerService) -> Self {
        Self { docker }
    }

    #[instrument(skip(self))]
    pub async fn get_status(&self, verbose: bool) -> Result<SystemStatus> {
        debug!("Gathering system status");

        let mut system_status = SystemStatus {
            services: HashMap::new(),
            timestamp: OffsetDateTime::now_utc(),
        };

        // Get Docker service status
        let docker_statuses = self.docker.get_service_status().await?;

        // Core services to check
        let core_services = [
            "pds",
            "plc",
            "appview",
            "bgs",
            "social-app",
            "ozone",
            "feed-generator",
            "jetstream",
        ];

        for service_name in core_services.iter() {
            let docker_status = docker_statuses.get(*service_name);

            let mut service_status = ServiceStatus {
                name: service_name.to_string(),
                running: docker_status.map(|s| s.running).unwrap_or(false),
                healthy: false,
                endpoint: None,
                version: None,
                details: HashMap::new(),
            };

            if verbose {
                // Add additional details for verbose output
                if let Some(ds) = docker_status {
                    service_status
                        .details
                        .insert("state".to_string(), ds.state.clone());
                    service_status.details.extend(
                        ds.ports
                            .iter()
                            .enumerate()
                            .map(|(i, p)| (format!("port_{}", i), p.to_string())),
                    );
                }
            }

            system_status
                .services
                .insert(service_name.to_string(), service_status);
        }

        Ok(system_status)
    }

    pub fn print_status(&self, status: &SystemStatus, verbose: bool) {
        println!("\n{}", "Service Status:".bold());
        println!("{}", "=============".bold());

        for (name, status) in &status.services {
            let status_indicator = if status.running {
                "✓".green().to_string()
            } else {
                "✗".red().to_string()
            };

            print!("{} {} ", status_indicator, name.bold());

            if verbose {
                println!();
                for (key, value) in &status.details {
                    println!("  {}: {}", key.yellow(), value);
                }
            } else {
                let state = if status.running {
                    "Running".green().to_string()
                } else {
                    "Stopped".red().to_string()
                };
                println!("- {}", state);
            }
        }

        // Format timestamp using time's formatting
        println!(
            "\nLast Updated: {}",
            status
                .timestamp
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap()
        );
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;

    #[tokio::test]
    async fn test_status_manager() {
        // Create a temporary compose file
        let temp = assert_fs::TempDir::new().unwrap();
        let compose_path = temp.child("docker-compose.yml");

        // Create a basic compose configuration
        let mut compose = crate::compose::ComposeConfig::new();
        compose.add_pds("test.com");
        compose.save(&compose_path).unwrap();

        // Create StatusManager
        let docker = crate::docker::DockerService::new(compose_path.to_str().unwrap());
        let status_manager = StatusManager::new(docker);

        // Get status
        let system_status = status_manager.get_status(true).await.unwrap();

        // Basic assertions
        assert!(system_status.services.contains_key("pds"));
        assert!(system_status.timestamp <= OffsetDateTime::now_utc());
    }

    #[tokio::test]
    async fn test_service_details() {
        let temp = assert_fs::TempDir::new().unwrap();
        let compose_path = temp.child("docker-compose.yml");

        // Create a more complete compose configuration
        let mut compose = crate::compose::ComposeConfig::new();
        compose
            .add_pds("test.com")
            .add_plc()
            .add_bgs()
            .add_appview();
        compose.save(&compose_path).unwrap();

        let docker = crate::docker::DockerService::new(compose_path.to_str().unwrap());
        let status_manager = StatusManager::new(docker);

        // Get verbose status
        let system_status = status_manager.get_status(true).await.unwrap();

        // Check all core services are present
        assert!(system_status.services.contains_key("pds"));
        assert!(system_status.services.contains_key("plc"));
        assert!(system_status.services.contains_key("bgs"));
        assert!(system_status.services.contains_key("appview"));
    }
}
