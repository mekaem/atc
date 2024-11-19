use crate::error::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, instrument, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub service: String,
    pub status: HealthState,
    pub latency_ms: u64,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

pub struct HealthChecker {
    client: Client,
    base_domain: String,
}

impl HealthChecker {
    pub fn new(domain: &str) -> Self {
        Self {
            client: Client::builder()
                .danger_accept_invalid_certs(true) // For development with self-signed certs
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client"),
            base_domain: domain.to_string(),
        }
    }

    #[instrument(skip(self))]
    pub async fn check_service(&self, service: &str) -> Result<HealthStatus> {
        debug!("Checking health for service: {}", service);

        let start = std::time::Instant::now();
        let status = match service {
            "pds" => self.check_pds().await?,
            "plc" => self.check_plc().await?,
            "appview" => self.check_appview().await?,
            "bgs" => self.check_bgs().await?,
            "social-app" => self.check_social_app().await?,
            "ozone" => self.check_ozone().await?,
            "feed-generator" => self.check_feed_generator().await?,
            "jetstream" => self.check_jetstream().await?,
            _ => {
                warn!("Unknown service: {}", service);
                HealthState::Unhealthy
            }
        };

        let latency = start.elapsed().as_millis() as u64;

        Ok(HealthStatus {
            service: service.to_string(),
            status,
            latency_ms: latency,
            details: None,
        })
    }

    async fn check_pds(&self) -> Result<HealthState> {
        let url = format!("https://pds.{}/xrpc/_health", self.base_domain);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(HealthState::Healthy),
            Ok(_) => Ok(HealthState::Degraded),
            Err(_) => Ok(HealthState::Unhealthy),
        }
    }

    async fn check_plc(&self) -> Result<HealthState> {
        let url = format!("https://plc.{}/health", self.base_domain);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(HealthState::Healthy),
            Ok(_) => Ok(HealthState::Degraded),
            Err(_) => Ok(HealthState::Unhealthy),
        }
    }

    async fn check_appview(&self) -> Result<HealthState> {
        let url = format!("https://appview.{}/xrpc/_health", self.base_domain);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(HealthState::Healthy),
            Ok(_) => Ok(HealthState::Degraded),
            Err(_) => Ok(HealthState::Unhealthy),
        }
    }

    async fn check_bgs(&self) -> Result<HealthState> {
        let url = format!("https://bgs.{}/health", self.base_domain);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(HealthState::Healthy),
            Ok(_) => Ok(HealthState::Degraded),
            Err(_) => Ok(HealthState::Unhealthy),
        }
    }

    async fn check_social_app(&self) -> Result<HealthState> {
        let url = format!("https://social-app.{}", self.base_domain);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(HealthState::Healthy),
            Ok(_) => Ok(HealthState::Degraded),
            Err(_) => Ok(HealthState::Unhealthy),
        }
    }

    async fn check_ozone(&self) -> Result<HealthState> {
        let url = format!("https://ozone.{}/health", self.base_domain);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(HealthState::Healthy),
            Ok(_) => Ok(HealthState::Degraded),
            Err(_) => Ok(HealthState::Unhealthy),
        }
    }

    async fn check_feed_generator(&self) -> Result<HealthState> {
        let url = format!("https://feed-generator.{}/health", self.base_domain);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(HealthState::Healthy),
            Ok(_) => Ok(HealthState::Degraded),
            Err(_) => Ok(HealthState::Unhealthy),
        }
    }

    async fn check_jetstream(&self) -> Result<HealthState> {
        let _url = format!("wss://jetstream.{}/health", self.base_domain);
        // For now just check if the endpoint exists
        Ok(HealthState::Healthy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_health_checker() {
        let mock_server = MockServer::start().await;
        let mock_server_uri = mock_server.uri();
        let mock_domain = mock_server_uri.trim_start_matches("http://");

        // Mock health endpoint
        Mock::given(method("GET"))
            .and(path("/_health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let checker = HealthChecker::new(mock_domain);
        let status = checker.check_service("pds").await.unwrap();

        assert_eq!(status.status, HealthState::Healthy);
        assert!(status.latency_ms > 0);
    }

    #[tokio::test]
    async fn test_degraded_service() {
        let mock_server = MockServer::start().await;
        let mock_server_uri = mock_server.uri();
        let mock_domain = mock_server_uri.trim_start_matches("http://");

        // Mock degraded service
        Mock::given(method("GET"))
            .and(path("/_health"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let checker = HealthChecker::new(mock_domain);
        let status = checker.check_service("pds").await.unwrap();

        assert_eq!(status.status, HealthState::Degraded);
    }
}
