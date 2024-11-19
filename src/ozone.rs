use crate::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

#[derive(Debug, Serialize)]
struct UpdateDidDocRequest {
    plc_sign_token: String,
    handle: String,
    ozone_url: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDidDocResponse {
    pub did: String,
    pub updated: bool,
}

pub struct OzoneClient {
    client: Client,
    base_url: String,
}

impl OzoneClient {
    pub fn new(domain: &str) -> Self {
        Self {
            client: Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Failed to create HTTP client"),
            base_url: domain.to_string(),
        }
    }

    #[instrument(skip(self))]
    pub async fn request_plc_sign(&self, handle: &str) -> Result<String> {
        let url = format!("{}/api/ozone/reqPlcSign", self.base_url);
        debug!("Requesting PLC sign for handle: {}", handle);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&serde_json::json!({
                "handle": handle
            }))?)
            .send()
            .await
            .map_err(|e| Error::Api(format!("Failed to request PLC sign: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api(format!(
                "Failed to request PLC sign: {}",
                error_text
            )));
        }

        let token = response
            .text()
            .await
            .map_err(|e| Error::Api(format!("Failed to read response: {}", e)))?;

        Ok(token)
    }

    #[instrument(skip(self))]
    pub async fn update_did_doc(
        &self,
        plc_sign_token: &str,
        handle: &str,
        ozone_url: &str,
    ) -> Result<UpdateDidDocResponse> {
        let url = format!("{}/api/ozone/updateDidDoc", self.base_url);
        debug!("Updating DID doc at {}", url);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "plc_sign_token": plc_sign_token,
                "handle": handle,
                "ozone_url": ozone_url,
            }))
            .send()
            .await
            .map_err(|e| Error::Api(format!("Failed to update DID doc: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api(format!(
                "Failed to update DID doc: {}",
                error_text
            )));
        }

        response
            .json::<UpdateDidDocResponse>()
            .await
            .map_err(|e| Error::Api(format!("Failed to parse response: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_request_plc_sign() {
        let mock_server = MockServer::start().await;
        let test_token = "test_plc_sign_token";

        Mock::given(method("POST"))
            .and(path("/api/ozone/reqPlcSign"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(test_token))
            .mount(&mock_server)
            .await;

        let client = OzoneClient {
            client: Client::new(),
            base_url: mock_server.uri(),
        };

        let token = client.request_plc_sign("test.handle").await.unwrap();
        assert_eq!(token, test_token);
    }

    #[tokio::test]
    async fn test_update_did_doc() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/ozone/updateDidDoc"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "did": "did:plc:test123",
                "updated": true
            })))
            .mount(&mock_server)
            .await;

        let client = OzoneClient {
            client: Client::new(),
            base_url: mock_server.uri(),
        };

        let response = client
            .update_did_doc("test_token", "test.handle", "https://ozone.test.com")
            .await
            .unwrap();

        assert_eq!(response.did, "did:plc:test123");
        assert!(response.updated);
    }
}
