use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Serialize, Deserialize)]
pub struct ComposeConfig {
    pub services: HashMap<String, Service>,
    pub networks: Option<HashMap<String, Network>>,
    pub volumes: Option<HashMap<String, Volume>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Service {
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Network {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Volume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
}

impl Service {
    fn new(image: &str) -> Self {
        Service {
            image: image.to_string(),
            ..Default::default()
        }
    }

    fn with_container_name(mut self, name: &str) -> Self {
        self.container_name = Some(name.to_string());
        self
    }

    fn with_restart(mut self, restart: &str) -> Self {
        self.restart = Some(restart.to_string());
        self
    }

    fn with_environment(mut self, env: Vec<&str>) -> Self {
        self.environment = Some(env.into_iter().map(String::from).collect());
        self
    }

    fn with_ports(mut self, ports: Vec<&str>) -> Self {
        self.ports = Some(ports.into_iter().map(String::from).collect());
        self
    }

    fn with_volumes(mut self, volumes: Vec<&str>) -> Self {
        self.volumes = Some(volumes.into_iter().map(String::from).collect());
        self
    }

    fn with_depends_on(mut self, deps: Vec<&str>) -> Self {
        self.depends_on = Some(deps.into_iter().map(String::from).collect());
        self
    }

    fn with_networks(mut self, networks: Vec<&str>) -> Self {
        self.networks = Some(networks.into_iter().map(String::from).collect());
        self
    }
}

impl ComposeConfig {
    pub fn new() -> Self {
        let mut networks = HashMap::new();
        networks.insert(
            "bluesky".to_string(),
            Network {
                external: None,
                driver: Some("bridge".to_string()),
            },
        );

        let mut volumes = HashMap::new();
        for vol in [
            "caddy_data",
            "caddy_config",
            "pds_data",
            "bgs_data",
            "postgres_data",
        ] {
            volumes.insert(
                vol.to_string(),
                Volume {
                    external: None,
                    driver: None,
                },
            );
        }

        Self {
            services: HashMap::new(),
            networks: Some(networks),
            volumes: Some(volumes),
        }
    }

    pub fn add_caddy(&mut self) -> &mut Self {
        let service = Service::new("caddy:2")
            .with_container_name("caddy")
            .with_restart("unless-stopped")
            .with_ports(vec!["80:80", "443:443"])
            .with_volumes(vec![
                "./config/caddy/Caddyfile:/etc/caddy/Caddyfile",
                "./certs:/etc/ssl/certs:ro",
                "caddy_data:/data",
                "caddy_config:/config",
            ])
            .with_networks(vec!["bluesky"]);

        self.services.insert("caddy".to_string(), service);
        self
    }

    pub fn add_pds(&mut self, domain: &str) -> &mut Self {
        let service = Service::new("ghcr.io/bluesky-social/pds:latest")
            .with_container_name("pds")
            .with_restart("unless-stopped")
            .with_environment(vec![
                &format!("PDS_HOSTNAME=pds.{}", domain),
                "PDS_JWT_SECRET=${PDS_JWT_SECRET}",
                "PDS_ADMIN_PASSWORD=${PDS_ADMIN_PASSWORD}",
                "PDS_PLC_ROTATION_KEY_K256=${PDS_PLC_ROTATION_KEY_K256}",
                "PDS_DATA_DIRECTORY=/data",
            ])
            .with_volumes(vec!["pds_data:/data"])
            .with_depends_on(vec!["caddy"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("pds".to_string(), service);
        self
    }

    pub fn add_plc(&mut self) -> &mut Self {
        let service = Service::new("ghcr.io/bluesky-social/plc:latest")
            .with_container_name("plc")
            .with_restart("unless-stopped")
            .with_depends_on(vec!["caddy"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("plc".to_string(), service);
        self
    }

    pub fn add_bgs(&mut self) -> &mut Self {
        let service = Service::new("ghcr.io/bluesky-social/bgs:latest")
            .with_container_name("bgs")
            .with_restart("unless-stopped")
            .with_environment(vec![
                "BGS_SUBSCRIBE_REPOS=wss://pds:2470",
                "BGS_SUBSCRIBE_SEQ_SCAN_INTERVAL=60m",
            ])
            .with_ports(vec!["2470:2470"])
            .with_volumes(vec!["bgs_data:/data"])
            .with_depends_on(vec!["pds"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("bgs".to_string(), service);
        self
    }

    pub fn add_appview(&mut self) -> &mut Self {
        let service = Service::new("ghcr.io/bluesky-social/appview:latest")
            .with_container_name("appview")
            .with_restart("unless-stopped")
            .with_environment(vec![
                "APPVIEW_SUBSCRIBE_REPOS=wss://pds:2470",
                "APPVIEW_SUBSCRIBE_FROM_SEQ=0",
                "APPVIEW_DATABASE_URL=postgres://postgres:postgres@db:5432/appview",
            ])
            .with_ports(vec!["3000:3000"])
            .with_depends_on(vec!["pds", "db"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("appview".to_string(), service);
        self.add_db()
    }

    pub fn add_db(&mut self) -> &mut Self {
        let service = Service::new("postgres:15-alpine")
            .with_container_name("db")
            .with_restart("unless-stopped")
            .with_environment(vec![
                "POSTGRES_USER=postgres",
                "POSTGRES_PASSWORD=postgres",
                "POSTGRES_DB=appview",
            ])
            .with_ports(vec!["5432:5432"])
            .with_volumes(vec!["postgres_data:/var/lib/postgresql/data"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("db".to_string(), service);
        self
    }

    pub fn add_feed_generator(&mut self, publisher_did: &str) -> &mut Self {
        let service = Service::new("ghcr.io/bluesky-social/feed-generator:latest")
            .with_container_name("feed-generator")
            .with_restart("unless-stopped")
            .with_environment(vec![
                &format!("FEEDGEN_PUBLISHER_DID={}", publisher_did),
                "FEEDGEN_HOSTNAME=feed-generator.${DOMAIN}",
                "FEEDGEN_SUBSCRIPTION_ENDPOINT=wss://bgs.${DOMAIN}",
                "FEEDGEN_SUBSCRIPTION_RECONNECT_DELAY=200",
            ])
            .with_ports(vec!["3000:3000"])
            .with_depends_on(vec!["bgs"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("feed-generator".to_string(), service);
        self
    }

    pub fn add_ozone(&mut self, server_did: &str, admin_dids: &[String]) -> &mut Self {
        let service = Service::new("ghcr.io/bluesky-social/ozone:latest")
            .with_container_name("ozone")
            .with_restart("unless-stopped")
            .with_environment(vec![
                &format!("OZONE_SERVER_DID={}", server_did),
                &format!("OZONE_ADMIN_DIDS={}", admin_dids.join(",")),
                "OZONE_PLC_HOST=http://plc:2582",
                "OZONE_APP_VIEW_HOST=http://appview:3000",
                "OZONE_DATABASE_URL=postgres://postgres:postgres@db:5432/ozone",
            ])
            .with_ports(vec!["3000:3000"])
            .with_depends_on(vec!["plc", "appview", "db"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("ozone".to_string(), service);

        // Update DB environment for multiple databases
        if let Some(db) = self.services.get_mut("db") {
            if let Some(env) = &mut db.environment {
                env.push("POSTGRES_MULTIPLE_DATABASES=appview,ozone".to_string());
            }
        }

        self
    }

    pub fn add_jetstream(&mut self, reconnect_delay: u32) -> &mut Self {
        let service = Service::new("ghcr.io/bluesky-social/jetstream:latest")
            .with_container_name("jetstream")
            .with_restart("unless-stopped")
            .with_environment(vec![
                "JETSTREAM_SUBSCRIPTION_ENDPOINT=wss://bgs.${DOMAIN}",
                &format!("JETSTREAM_SUBSCRIPTION_RECONNECT_DELAY={}", reconnect_delay),
            ])
            .with_ports(vec!["3000:3000"])
            .with_depends_on(vec!["bgs"])
            .with_networks(vec!["bluesky"]);

        self.services.insert("jetstream".to_string(), service);
        self
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = serde_yaml::to_string(self).map_err(|e| {
            crate::error::Error::Yaml(format!("Failed to serialize compose config: {}", e))
        })?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content).map_err(|e| {
            crate::error::Error::Yaml(format!("Failed to parse compose config: {}", e))
        })?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compose_config_creation() {
        let config = ComposeConfig::new();
        assert_eq!(config.services.len(), 0);
        assert!(config.networks.is_some());
        assert!(config.volumes.is_some());
    }

    #[test]
    fn test_service_builder() {
        let service = Service::new("test:latest")
            .with_container_name("test")
            .with_restart("always")
            .with_environment(vec!["KEY=VALUE"])
            .with_ports(vec!["8080:80"])
            .with_volumes(vec!["/host:/container"])
            .with_depends_on(vec!["db"])
            .with_networks(vec!["test_net"]);

        assert_eq!(service.image, "test:latest");
        assert_eq!(service.container_name, Some("test".to_string()));
        assert_eq!(service.restart, Some("always".to_string()));
        assert_eq!(service.environment, Some(vec!["KEY=VALUE".to_string()]));
        assert_eq!(service.ports, Some(vec!["8080:80".to_string()]));
        assert_eq!(service.volumes, Some(vec!["/host:/container".to_string()]));
        assert_eq!(service.depends_on, Some(vec!["db".to_string()]));
        assert_eq!(service.networks, Some(vec!["test_net".to_string()]));
    }

    #[test]
    fn test_complete_stack() {
        let mut config = ComposeConfig::new();
        config
            .add_caddy()
            .add_pds("example.com")
            .add_plc()
            .add_bgs()
            .add_appview();

        // Verify all required services are present
        assert!(config.services.contains_key("caddy"));
        assert!(config.services.contains_key("pds"));
        assert!(config.services.contains_key("plc"));
        assert!(config.services.contains_key("bgs"));
        assert!(config.services.contains_key("appview"));
        assert!(config.services.contains_key("db"));

        // Verify volumes
        let volumes = config.volumes.as_ref().unwrap();
        assert!(volumes.contains_key("bgs_data"));
        assert!(volumes.contains_key("postgres_data"));
    }

    #[test]
    fn test_compose_roundtrip() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("docker-compose.yml");

        let mut config = ComposeConfig::new();
        config
            .add_caddy()
            .add_pds("test.com")
            .add_plc()
            .add_bgs()
            .add_appview();

        config.save(&config_path)?;
        let loaded = ComposeConfig::load(&config_path)?;

        assert_eq!(config.services.len(), loaded.services.len());
        assert_eq!(config.services.len(), loaded.services.len());
        assert!(loaded.services.contains_key("caddy"));
        assert!(loaded.services.contains_key("pds"));
        assert!(loaded.services.contains_key("plc"));

        // Verify service configurations are preserved
        let pds = loaded.services.get("pds").unwrap();
        assert!(pds
            .environment
            .as_ref()
            .unwrap()
            .iter()
            .any(|e| e.contains("test.com")));

        Ok(())
    }

    #[test]
    fn test_add_feed_generator() {
        let mut config = ComposeConfig::new();
        config.add_feed_generator("did:plc:test123");

        let feed_gen = config.services.get("feed-generator").unwrap();
        assert_eq!(
            feed_gen.image,
            "ghcr.io/bluesky-social/feed-generator:latest"
        );
        assert!(feed_gen
            .environment
            .as_ref()
            .unwrap()
            .iter()
            .any(|e| e.contains("did:plc:test123")));
    }

    #[test]
    fn test_add_ozone() {
        let mut config = ComposeConfig::new();
        config.add_appview(); // This adds db service too
        config.add_ozone("did:plc:test123", &vec!["did:plc:admin456".to_string()]);

        let ozone = config.services.get("ozone").unwrap();
        assert_eq!(ozone.image, "ghcr.io/bluesky-social/ozone:latest");

        let env = ozone.environment.as_ref().unwrap();
        assert!(env.iter().any(|e| e.contains("did:plc:test123")));
        assert!(env.iter().any(|e| e.contains("did:plc:admin456")));

        // Check database configuration update
        let db = config.services.get("db").unwrap();
        assert!(db
            .environment
            .as_ref()
            .unwrap()
            .contains(&"POSTGRES_MULTIPLE_DATABASES=appview,ozone".to_string()));
    }

    #[test]
    fn test_add_jetstream() {
        let mut config = ComposeConfig::new();
        config.add_jetstream(200);

        let jetstream = config.services.get("jetstream").unwrap();
        assert_eq!(jetstream.image, "ghcr.io/bluesky-social/jetstream:latest");
        assert!(jetstream
            .environment
            .as_ref()
            .unwrap()
            .iter()
            .any(|e| e.contains("RECONNECT_DELAY=200")));
    }
}
