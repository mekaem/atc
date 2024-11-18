use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Config {
    pub network: NetworkConfig,
    pub storage: StorageConfig,
    pub email: EmailConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkConfig {
    pub domain: String,
    pub bind_address: String,
    pub use_tls: bool,
    pub ports: Ports,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ports {
    pub http: u16,
    pub https: u16,
    pub pds: u16,
    pub plc: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    pub cert_dir: PathBuf,
    pub persist_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmailConfig {
    pub smtp_url: String,
    pub cert_email: String,
    pub admin_email: String,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| Error::Config(format!("Failed to read config file: {e}")))?;

        toml::from_str(&content).map_err(|e| Error::Config(format!("Failed to parse config: {e}")))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {e}")))?;

        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.network.domain.is_empty() {
            return Err(Error::Config("Domain cannot be empty".into()));
        }

        if self.network.bind_address.is_empty() {
            return Err(Error::Config("Bind address cannot be empty".into()));
        }

        // Validate port uniqueness
        let ports = &self.network.ports;
        let port_values = [ports.http, ports.https, ports.pds, ports.plc];
        let unique_ports: std::collections::HashSet<_> = port_values.iter().collect();

        if unique_ports.len() != port_values.len() {
            return Err(Error::Config("Port numbers must be unique".into()));
        }

        Ok(())
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            domain: "localhost".into(),
            bind_address: "0.0.0.0".into(),
            use_tls: true,
            ports: Ports::default(),
        }
    }
}

impl Default for Ports {
    fn default() -> Self {
        Self {
            http: 80,
            https: 443,
            pds: 2583,
            plc: 2582,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            cert_dir: PathBuf::from("./certs"),
            persist_data: true,
        }
    }
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_url: String::new(),
            cert_email: "admin@localhost".into(),
            admin_email: "admin@localhost".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.network.domain, "localhost");
        assert_eq!(config.network.ports.http, 80);
        assert!(config.storage.persist_data);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        // Test empty domain
        config.network.domain = "".into();
        assert!(config.validate().is_err());

        // Test duplicate ports
        config = Config::default();
        config.network.ports.http = config.network.ports.https;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_save_and_load() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("config.toml");

        let config = Config::default();
        config.save(&config_path)?;

        let loaded_config = Config::load(&config_path)?;
        assert_eq!(config, loaded_config);

        Ok(())
    }

    #[test]
    fn test_config_round_trip() -> Result<()> {
        let config = Config {
            network: NetworkConfig {
                domain: "test.com".into(),
                bind_address: "127.0.0.1".into(),
                use_tls: false,
                ports: Ports {
                    http: 8080,
                    https: 8443,
                    pds: 3000,
                    plc: 3001,
                },
            },
            storage: StorageConfig {
                data_dir: PathBuf::from("/tmp/data"),
                cert_dir: PathBuf::from("/tmp/certs"),
                persist_data: false,
            },
            email: EmailConfig {
                smtp_url: "smtp://localhost:25".into(),
                cert_email: "cert@test.com".into(),
                admin_email: "admin@test.com".into(),
            },
        };

        let dir = tempdir()?;
        let config_path = dir.path().join("config.toml");

        config.save(&config_path)?;
        let content = fs::read_to_string(&config_path)?;
        let loaded_config: Config = toml::from_str(&content)?;

        assert_eq!(config, loaded_config);
        Ok(())
    }
}
