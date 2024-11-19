use crate::error::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct JetstreamConfig {
    pub collections: Vec<String>,
    pub subscription_endpoint: String,
    pub reconnect_delay: u32,
}

pub struct JetstreamClient {
    client: Client,
    base_url: String,
}

impl JetstreamClient {
    pub fn new(domain: &str) -> Self {
        Self {
            client: Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Failed to create HTTP client"),
            base_url: format!("https://jetstream.{}", domain),
        }
    }

    #[instrument(skip(self))]
    pub async fn subscribe(&self, collections: &[String]) -> Result<()> {
        let url = format!(
            "wss://jetstream.{}/subscribe?{}",
            self.base_url,
            collections
                .iter()
                .map(|c| format!("wantedCollections={}", c))
                .collect::<Vec<_>>()
                .join("&")
        );
        debug!("Subscribing to collections at: {}", url);

        // TODO! Implement WebSocket connection
        Ok(())
    }
}

// Standard collections available in Jetstream
pub const STANDARD_COLLECTIONS: &[&str] = &[
    "app.bsky.actor.profile",
    "app.bsky.feed.like",
    "app.bsky.feed.post",
    "app.bsky.feed.repost",
    "app.bsky.graph.follow",
    "app.bsky.graph.block",
    "app.bsky.graph.muteActor",
    "app.bsky.graph.unmuteActor",
];

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_jetstream_client() {
        let mock_server = MockServer::start().await;
        let mock_server_uri = mock_server.uri();
        let domain = mock_server_uri.trim_start_matches("http://");

        let client = JetstreamClient::new(domain);
        assert!(client.base_url.contains(domain));
    }
}
