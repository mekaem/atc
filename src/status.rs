use crate::docker::DockerServiceTrait;
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

pub struct StatusManager<T: DockerServiceTrait> {
    docker: T,
}

impl<T: DockerServiceTrait> StatusManager<T> {
    pub fn new(docker: T) -> Self {
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
    use crate::docker::{mock::MockDockerService, ServiceStatus as DockerServiceStatus};

    async fn setup_mock_docker() -> MockDockerService {
        let docker = MockDockerService::new();

        // Set up some mock services
        docker
            .set_service_status(
                "pds",
                DockerServiceStatus {
                    running: true,
                    state: "running".to_string(),
                    ports: vec!["3000:3000".to_string()],
                },
            )
            .await;

        docker
            .set_service_status(
                "plc",
                DockerServiceStatus {
                    running: true,
                    state: "running".to_string(),
                    ports: vec!["2582:2582".to_string()],
                },
            )
            .await;

        docker
            .set_service_status(
                "bgs",
                DockerServiceStatus {
                    running: false,
                    state: "exited".to_string(),
                    ports: vec![],
                },
            )
            .await;

        docker
    }

    #[tokio::test]
    async fn test_status_manager() {
        let docker = setup_mock_docker().await;
        let status_manager = StatusManager::new(docker);

        let system_status = status_manager.get_status(true).await.unwrap();

        // Verify PDS service status
        let pds_status = system_status.services.get("pds").unwrap();
        assert!(pds_status.running);
        assert_eq!(pds_status.name, "pds");

        // Verify BGS service status (not running)
        let bgs_status = system_status.services.get("bgs").unwrap();
        assert!(!bgs_status.running);
    }

    #[tokio::test]
    async fn test_service_details() {
        let docker = setup_mock_docker().await;
        let status_manager = StatusManager::new(docker);

        // Test verbose mode
        let system_status = status_manager.get_status(true).await.unwrap();

        // Check for detailed port information
        let pds_status = system_status.services.get("pds").unwrap();
        assert!(pds_status.details.get("port_0").unwrap().contains("3000"));

        // Test non-verbose mode
        let system_status = status_manager.get_status(false).await.unwrap();
        let pds_status = system_status.services.get("pds").unwrap();
        assert!(pds_status.details.is_empty());
    }

    #[tokio::test]
    async fn test_timestamp_accuracy() {
        let docker = setup_mock_docker().await;
        let status_manager = StatusManager::new(docker);

        let before = OffsetDateTime::now_utc();
        let system_status = status_manager.get_status(false).await.unwrap();
        let after = OffsetDateTime::now_utc();

        assert!(system_status.timestamp >= before);
        assert!(system_status.timestamp <= after);
    }

    #[tokio::test]
    async fn test_all_core_services_present() {
        let docker = setup_mock_docker().await;
        let status_manager = StatusManager::new(docker);

        let system_status = status_manager.get_status(false).await.unwrap();

        // Check that all core services are present in the status
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

        for service in core_services.iter() {
            assert!(
                system_status.services.contains_key(*service),
                "Missing core service: {}",
                service
            );
        }
    }

    #[tokio::test]
    async fn test_verbose_output_format() {
        let docker = setup_mock_docker().await;
        let status_manager = StatusManager::new(docker);

        let system_status = status_manager.get_status(true).await.unwrap();

        // Check PDS service details
        let pds_status = system_status.services.get("pds").unwrap();
        assert!(pds_status.details.contains_key("state"));
        assert!(pds_status.details.contains_key("port_0"));
        assert_eq!(pds_status.details.get("state").unwrap(), "running");
        assert_eq!(pds_status.details.get("port_0").unwrap(), "3000:3000");

        // Check BGS service details (not running)
        let bgs_status = system_status.services.get("bgs").unwrap();
        assert_eq!(bgs_status.details.get("state").unwrap(), "exited");
        assert!(!bgs_status.details.contains_key("port_0"));
    }

    #[tokio::test]
    async fn test_service_health_tracking() {
        let docker = setup_mock_docker().await;
        let status_manager = StatusManager::new(docker);

        let system_status = status_manager.get_status(true).await.unwrap();

        // Initially all services should be marked as not healthy
        for (_, status) in system_status.services.iter() {
            assert!(
                !status.healthy,
                "Service {} should initially be marked as not healthy",
                status.name
            );
        }
    }

    #[tokio::test]
    async fn test_empty_service_status() {
        let docker = MockDockerService::new();
        let status_manager = StatusManager::new(docker);

        let system_status = status_manager.get_status(true).await.unwrap();

        // All services should be present but marked as not running
        for (_, status) in system_status.services.iter() {
            assert!(!status.running);
            assert!(status.details.is_empty());
        }
    }
}
