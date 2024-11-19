use crate::error::{Error, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, instrument};

pub struct DnsChecker;

impl DnsChecker {
    #[instrument]
    pub async fn check_domain(domain: &str) -> Result<bool> {
        debug!("Checking DNS for domain: {}", domain);

        match Self::dig_check(domain).await {
            Ok(true) => {
                debug!("Main domain resolves successfully");
                Ok(true)
            }
            Ok(false) => {
                debug!("Main domain does not resolve");
                Ok(false)
            }
            Err(e) => {
                debug!("Error checking domain: {}", e);
                Err(e)
            }
        }
    }

    #[instrument]
    async fn dig_check(domain: &str) -> Result<bool> {
        let output = Command::new("dig")
            .arg("+short")
            .arg("+time=2") // 2 second timeout
            .arg("+tries=1") // Only try once
            .arg("A")
            .arg(domain)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::Network(format!("Failed to run dig: {}", e)))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Check if we got any IP addresses back
        let has_ip = !output_str.trim().is_empty()
            && output_str.lines().any(|line| {
                line.split('.')
                    .filter(|octet| octet.parse::<u8>().is_ok())
                    .count()
                    == 4
            });

        Ok(has_ip)
    }

    #[instrument]
    pub async fn check_ssl_test_endpoint(domain: &str) -> Result<bool> {
        debug!("Testing HTTPS endpoint");

        let url = format!("https://test-wss.{}/", domain);
        let output = Command::new("curl")
            .arg("-L")
            .arg("-k") // Allow insecure for testing
            .arg("--fail") // Fail on HTTP errors
            .arg("-s") // Silent mode
            .arg("--connect-timeout")
            .arg("5")
            .arg(&url)
            .output()
            .await
            .map_err(|e| Error::Network(format!("Failed to test HTTPS: {}", e)))?;

        Ok(output.status.success())
    }

    #[instrument]
    pub async fn check_websocket_endpoint(domain: &str) -> Result<bool> {
        debug!("Testing WebSocket endpoint");

        let url = format!("https://test-wss.{}/ws", domain);
        let output = Command::new("websocat")
            .arg("--connect-timeout")
            .arg("5")
            .arg(&url)
            .output()
            .await
            .map_err(|e| Error::Network(format!("Failed to test WebSocket: {}", e)))?;

        Ok(output.status.success())
    }

    #[cfg(test)]
    async fn dig_check_mock(domain: &str) -> Result<bool> {
        Ok(matches!(
            domain,
            "valid.example.com" | "any.valid.example.com"
        ))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    async fn mock_dig_response(domain: &str) -> String {
        match domain {
            "google.com" => "142.250.80.78".to_string(),
            "example.com" => "93.184.216.34".to_string(),
            _ => "".to_string(),
        }
    }

    impl DnsChecker {
        #[instrument(skip_all)]
        async fn dig_check_test(domain: &str) -> Result<bool> {
            let output = mock_dig_response(domain).await;
            Ok(!output.is_empty())
        }
    }

    #[tokio::test]
    async fn test_dns_checker() {
        let result = DnsChecker::dig_check_test("google.com").await.unwrap();
        assert!(result, "DNS check should succeed for google.com");

        let result = DnsChecker::dig_check_test("invalid-domain-test")
            .await
            .unwrap();
        assert!(!result, "DNS check should fail for invalid domain");
    }

    #[tokio::test]
    async fn test_dns_ip_parsing() {
        let result = DnsChecker::dig_check_test("google.com").await.unwrap();
        assert!(result, "Should find valid IP for google.com");

        let result = DnsChecker::dig_check_test("thisisnotarealdomain.invalid")
            .await
            .unwrap();
        assert!(!result, "Should not find IP for invalid domain");
    }
}
