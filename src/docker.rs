use crate::error::{Error, Result};
use crate::secrets::Secrets;
use async_trait::async_trait;
use serde::Deserialize;
use std::process::Stdio;
use std::{collections::HashMap, path::Path};
use tokio::process::Command;
use tracing::{debug, instrument};

#[async_trait]
pub trait DockerServiceTrait {
    async fn get_service_status(&self) -> Result<HashMap<String, ServiceStatus>>;
}

#[derive(Debug)]
pub struct DockerService {
    compose_file: String,
    env_vars: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub running: bool,
    pub state: String,
    pub ports: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DockerComposeService {
    name: String,
    state: String,
    ports: Vec<String>,
}

impl DockerService {
    pub fn new(compose_file: impl Into<String>) -> Self {
        Self {
            compose_file: compose_file.into(),
            env_vars: HashMap::new(),
        }
    }

    pub fn with_env_vars(mut self, env_vars: HashMap<String, String>) -> Self {
        self.env_vars = env_vars;
        self
    }

    #[instrument]
    pub async fn start_services(&self, services: Option<&[String]>) -> Result<()> {
        // Load secrets if they exist
        let mut env_vars = self.env_vars.clone();
        if Path::new("config/secrets.toml").exists() {
            let secrets = Secrets::load("config/secrets.toml")?;
            env_vars.extend(secrets.as_env_vars().into_iter());
        }

        let mut cmd = Command::new("docker-compose");
        cmd.arg("-f")
            .arg(&self.compose_file)
            .arg("up")
            .arg("-d")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // Add environment variables including secrets
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // Add specific services if requested
        if let Some(services) = services {
            cmd.args(services);
        }

        debug!("Running docker-compose command: {:?}", cmd);
        let status = cmd.status().await?;

        if !status.success() {
            return Err(Error::Docker("Failed to start services".into()));
        }

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn stop_services(&self, clean: bool) -> Result<()> {
        let mut cmd = Command::new("docker-compose");
        cmd.arg("-f")
            .arg(&self.compose_file)
            .arg("down")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        if clean {
            cmd.arg("-v"); // Remove volumes
        }

        debug!("Running docker-compose command: {:?}", cmd);
        let status = cmd.status().await?;

        if !status.success() {
            return Err(Error::Docker("Failed to stop services".into()));
        }

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_service_status(&self) -> Result<HashMap<String, ServiceStatus>> {
        let output = Command::new("docker-compose")
            .arg("-f")
            .arg(&self.compose_file)
            .arg("ps")
            .arg("--format")
            .arg("json")
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::Docker("Failed to get service status".into()));
        }

        let output = String::from_utf8_lossy(&output.stdout);
        let services: Vec<DockerComposeService> = output
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        let mut statuses = HashMap::new();
        for service in services {
            statuses.insert(
                service.name,
                ServiceStatus {
                    running: service.state == "running",
                    state: service.state,
                    ports: service.ports,
                },
            );
        }

        Ok(statuses)
    }

    #[instrument]
    pub async fn check_dependencies() -> Result<()> {
        // Check docker
        let docker_version = Command::new("docker").arg("--version").output().await?;

        if !docker_version.status.success() {
            return Err(Error::Docker("Docker is not installed".into()));
        }

        // Check docker-compose
        let compose_version = Command::new("docker-compose")
            .arg("--version")
            .output()
            .await?;

        if !compose_version.status.success() {
            return Err(Error::Docker("Docker Compose is not installed".into()));
        }

        Ok(())
    }
}

#[async_trait]
impl DockerServiceTrait for DockerService {
    async fn get_service_status(&self) -> Result<HashMap<String, ServiceStatus>> {
        self.get_service_status().await
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Debug, Clone, Default)]
    pub struct MockDockerService {
        services: Arc<Mutex<HashMap<String, ServiceStatus>>>,
    }

    impl MockDockerService {
        pub fn new() -> Self {
            Self {
                services: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub async fn set_service_status(&self, name: &str, status: ServiceStatus) {
            let mut services = self.services.lock().await;
            services.insert(name.to_string(), status);
        }
    }

    #[async_trait]
    impl DockerServiceTrait for MockDockerService {
        async fn get_service_status(&self) -> Result<HashMap<String, ServiceStatus>> {
            let services = self.services.lock().await;
            Ok(services.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docker_service_creation() {
        let docker = DockerService::new("docker-compose.yml");
        assert_eq!(docker.compose_file, "docker-compose.yml");
        assert!(docker.env_vars.is_empty());
    }

    #[tokio::test]
    async fn test_docker_service_with_env_vars() {
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

        let docker = DockerService::new("docker-compose.yml").with_env_vars(env_vars);
        assert_eq!(docker.env_vars.get("TEST_VAR").unwrap(), "test_value");
    }

    #[tokio::test]
    async fn test_mock_docker_service() {
        let docker = mock::MockDockerService::new();

        // Set up test service
        docker
            .set_service_status(
                "test-service",
                ServiceStatus {
                    running: true,
                    state: "running".to_string(),
                    ports: vec!["8080:80".to_string()],
                },
            )
            .await;

        // Get status and verify
        let statuses = docker.get_service_status().await.unwrap();
        let test_service = statuses.get("test-service").unwrap();
        assert!(test_service.running);
        assert_eq!(test_service.state, "running");
        assert_eq!(test_service.ports[0], "8080:80");
    }
}
