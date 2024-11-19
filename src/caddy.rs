use crate::error::Result;
use std::path::Path;
use tracing::instrument;

#[derive(Debug)]
pub struct CaddyConfig {
    domain: String,
}

impl CaddyConfig {
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
        }
    }

    #[instrument(skip(self))]
    pub fn generate(&self) -> String {
        let mut config = String::new();

        // Debug test endpoint
        config.push_str(&format!(
            "test-wss.{} {{
                respond \"OK\"
                handle /ws {{
                    respond \"OK\"
                }}
            }}

            ",
            self.domain
        ));

        // PDS configuration
        config.push_str(&format!(
            "*.pds.{domain}, pds.{domain} {{
                @api path /xrpc/*
                handle @api {{
                    reverse_proxy pds:3000
                }}
                handle * {{
                    reverse_proxy social-app:3000
                }}
            }}

            ",
            domain = self.domain
        ));

        // BGS configuration
        config.push_str(&format!(
            "*.bgs.{domain}, bgs.{domain} {{
                reverse_proxy bgs:2470
            }}

            ",
            domain = self.domain
        ));

        // Appview configuration
        config.push_str(&format!(
            "*.appview.{domain}, appview.{domain} {{
                reverse_proxy appview:3000
            }}

            ",
            domain = self.domain
        ));

        // PLC configuration
        config.push_str(&format!(
            "*.plc.{domain}, plc.{domain} {{
                reverse_proxy plc:2582
            }}

            ",
            domain = self.domain
        ));

        // Social app configuration
        config.push_str(&format!(
            "social-app.{domain} {{
                reverse_proxy social-app:3000
            }}

            ",
            domain = self.domain
        ));

        // Ozone configuration
        config.push_str(&format!(
            "ozone.{domain} {{
                reverse_proxy ozone:3000
            }}

            ",
            domain = self.domain
        ));

        config
    }
    #[instrument(skip(path))]
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, self.generate())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;

    #[test]
    fn test_generate_config() {
        let config = CaddyConfig::new("example.com");
        let content = config.generate();

        // Check that all required sections exist
        assert!(content.contains("test-wss.example.com"));
        assert!(content.contains("*.pds.example.com, pds.example.com"));
        assert!(content.contains("*.bgs.example.com, bgs.example.com"));
        assert!(content.contains("*.plc.example.com, plc.example.com"));
        assert!(content.contains("social-app.example.com"));
    }

    #[test]
    fn test_save_config() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp = assert_fs::TempDir::new()?;
        let config_path = temp.child("Caddyfile");

        let config = CaddyConfig::new("example.com");
        config.save(&config_path)?;

        config_path.assert(predicates::path::exists());

        let content = std::fs::read_to_string(config_path)?;
        assert!(content.contains("example.com"));

        Ok(())
    }

    #[test]
    fn test_proxy_rules() {
        let config = CaddyConfig::new("example.com");
        let content = config.generate();

        // Check PDS proxy rules
        assert!(content.contains("@api path /xrpc/*"));
        assert!(content.contains("reverse_proxy pds:3000"));
        assert!(content.contains("reverse_proxy social-app:3000"));

        // Check other service proxy rules
        assert!(content.contains("reverse_proxy bgs:2470"));
        assert!(content.contains("reverse_proxy plc:2582"));
        assert!(content.contains("reverse_proxy appview:3000"));
    }
}
