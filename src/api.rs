use crate::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

#[derive(Debug, Serialize)]
struct CreateAccountRequest {
    handle: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountResponse {
    pub did: String,
    pub handle: String,
}

pub struct PdsClient {
    client: Client,
    base_url: String,
}

impl PdsClient {
    pub fn new(domain: &str) -> Self {
        Self {
            client: Client::builder()
                .danger_accept_invalid_certs(true) // For self-signed certs
                .build()
                .expect("Failed to create HTTP client"),
            base_url: format!("https://pds.{}", domain),
        }
    }

    #[instrument(skip(self))]
    pub async fn create_account(
        &self,
        handle: String,
        email: String,
        password: String,
    ) -> Result<CreateAccountResponse> {
        let url = format!("{}/xrpc/com.atproto.server.createAccount", self.base_url);
        debug!("Creating account at: {}", url);

        let request = CreateAccountRequest {
            handle,
            email,
            password,
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&request)?)
            .send()
            .await
            .map_err(|e| Error::Api(format!("Failed to create account: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api(format!(
                "Failed to create account: {}",
                error_text
            )));
        }

        let account = response
            .json::<CreateAccountResponse>()
            .await
            .map_err(|e| Error::Api(format!("Failed to parse response: {}", e)))?;

        debug!("Account created successfully with DID: {}", account.did);
        Ok(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_create_account() {
        // Start mock server
        let mock_server = MockServer::start().await;

        // Get the mock server URL
        let mock_url = mock_server.uri();

        // Create a test client that points to the mock server
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        // Create a test PDS client with the mock URL
        let test_client = PdsClient {
            client,
            base_url: mock_url,
        };

        // Setup the mock
        Mock::given(method("POST"))
            .and(path("/xrpc/com.atproto.server.createAccount"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "did": "did:plc:test123",
                "handle": "test.example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Test the account creation
        let result = test_client
            .create_account(
                "test.example.com".to_string(),
                "test@example.com".to_string(),
                "password123".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(result.did, "did:plc:test123");
        assert_eq!(result.handle, "test.example.com");
    }

    #[tokio::test]
    async fn test_create_account_error() {
        let mock_server = MockServer::start().await;
        let mock_url = mock_server.uri();

        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let test_client = PdsClient {
            client,
            base_url: mock_url,
        };

        Mock::given(method("POST"))
            .and(path("/xrpc/com.atproto.server.createAccount"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": "Invalid handle"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = test_client
            .create_account(
                "invalid@handle".to_string(),
                "test@example.com".to_string(),
                "password123".to_string(),
            )
            .await;

        assert!(result.is_err());
    }
}
