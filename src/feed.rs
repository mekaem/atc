use crate::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

#[derive(Debug, Serialize)]
struct PublishFeedRequest {
    pub feed_did: String,
    pub feed_url: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishFeedResponse {
    pub uri: String,
    pub cid: String,
}

pub struct FeedGenerator {
    client: Client,
    base_url: String,
    did: String,
}

impl FeedGenerator {
    pub fn new(domain: &str, did: &str) -> Self {
        Self {
            client: Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Failed to create HTTP client"),
            base_url: format!("https://feed-generator.{}", domain),
            did: did.to_string(),
        }
    }

    #[instrument(skip(self))]
    pub async fn publish_feed(&self) -> Result<PublishFeedResponse> {
        let url = format!("{}/scripts/publishFeedGen.ts", self.base_url);
        debug!("Publishing feed at: {}", url);

        let request = PublishFeedRequest {
            feed_did: self.did.clone(),
            feed_url: format!("https://feed-generator.{}/", self.base_url),
            name: "test-feed".to_string(),
            display_name: "Test Feed".to_string(),
            description: "A test feed generator".to_string(),
            avatar: None,
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&request)?)
            .send()
            .await
            .map_err(|e| Error::Api(format!("Failed to publish feed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api(format!(
                "Failed to publish feed: {}",
                error_text
            )));
        }

        let feed = response
            .json::<PublishFeedResponse>()
            .await
            .map_err(|e| Error::Api(format!("Failed to parse response: {}", e)))?;

        debug!("Feed published successfully: {}", feed.uri);
        Ok(feed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_publish_feed() {
        let mock_server = MockServer::start().await;
        let mock_url = mock_server.uri();

        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let test_client = FeedGenerator {
            client,
            base_url: mock_url,
            did: "did:plc:test123".to_string(),
        };

        Mock::given(method("POST"))
            .and(path("/scripts/publishFeedGen.ts"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "uri": "at://did:plc:test123/app.bsky.feed.generator/test-feed",
                "cid": "bafyreia3tbsfxe3cc4aygxhkr2fr3oweenysz7tailjz6e3qgxgd6gqyra"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = test_client.publish_feed().await.unwrap();
        assert!(result.uri.contains("test-feed"));
        assert!(result.cid.starts_with("bafyrei"));
    }
}
