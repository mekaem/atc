use crate::{
    api::PdsClient,
    caddy::CaddyConfig,
    certs::CertManager,
    cli::Commands,
    compose::ComposeConfig,
    config::Config,
    dns::DnsChecker,
    docker::DockerService,
    error::{Error, Result},
    feed::FeedGenerator,
    ozone::OzoneClient,
    secrets::Secrets,
};
use owo_colors::OwoColorize;
use std::{fs, path::Path};
use tracing::{info, warn};

pub async fn handle_command(cmd: Commands, config_path: &Path) -> Result<()> {
    match cmd {
        Commands::Init(args) => {
            info!("Initializing new Bluesky configuration...");
            let mut config = Config::default();
            config.network.domain = args.domain;
            config.email.cert_email = args.cert_email;

            // Generate secrets
            info!("Generating secrets...");
            let secrets = Secrets::generate();
            fs::create_dir_all("config")?;
            secrets.save("config/secrets.toml")?;
            println!("{}", "Secrets generated successfully!".green());

            // Create directories
            fs::create_dir_all("config/caddy")?;
            fs::create_dir_all("certs")?;
            fs::create_dir_all("data")?;

            // Create docker-compose.yml
            let mut compose = ComposeConfig::new();
            let mut env_vars = create_env_vars(&config);
            env_vars.extend(secrets.as_env_vars().into_iter());

            compose
                .add_caddy()
                .add_pds(&config.network.domain)
                .add_plc()
                .add_bgs()
                .add_appview();

            // Generate Caddyfile
            let caddy = CaddyConfig::new(&config.network.domain);
            caddy.save("config/caddy/Caddyfile")?;
            println!("{}", "Caddyfile generated successfully!".green());

            // Save configs
            config.save(config_path)?;
            compose.save("docker-compose.yml")?;

            println!("{}", "Configuration created successfully!".green());
            Ok(())
        }

        Commands::Start(args) => {
            let config = Config::load(config_path)?;
            info!("Starting services...");

            if !args.no_deps {
                DockerService::check_dependencies().await?;
            }

            // Ensure compose file exists
            let compose_path = "docker-compose.yml";
            if !Path::new(compose_path).exists() {
                return Err(Error::Docker(format!(
                    "Docker Compose file not found: {}. Run init first.",
                    compose_path
                )));
            }

            // Start services using the compose file
            let docker = DockerService::new(compose_path).with_env_vars(create_env_vars(&config));
            docker.start_services(args.services.as_deref()).await?;
            println!("{}", "Services started successfully!".green());
            Ok(())
        }

        Commands::Stop(args) => {
            let compose_path = "docker-compose.yml";
            if !Path::new(compose_path).exists() {
                return Err(Error::Docker(format!(
                    "Docker Compose file not found: {}",
                    compose_path
                )));
            }

            let docker = DockerService::new(compose_path);

            if args.clean {
                warn!("Stopping services and cleaning data...");
                docker.stop_services(true).await?;
            } else {
                info!("Stopping services...");
                docker.stop_services(false).await?;
            }

            println!("{}", "Services stopped successfully!".green());
            Ok(())
        }

        Commands::Check(args) => {
            let config = Config::load(config_path)?;
            info!("Checking environment readiness...");

            // Check required files exist
            if !Path::new("docker-compose.yml").exists() {
                return Err(Error::Config(
                    "docker-compose.yml not found. Run init first.".into(),
                ));
            }

            if !Path::new("config/caddy").exists() {
                return Err(Error::Config("config/caddy directory not found".into()));
            }

            if !args.no_dns {
                info!("Checking DNS configuration...");
                if DnsChecker::check_domain(&config.network.domain).await? {
                    println!("{}", "DNS configuration: OK".green());
                } else {
                    return Err(Error::Network("DNS checks failed".into()));
                }

                info!("Testing HTTPS endpoint...");
                if DnsChecker::check_ssl_test_endpoint(&config.network.domain).await? {
                    println!("{}", "HTTPS endpoint: OK".green());
                } else {
                    return Err(Error::Network("HTTPS endpoint test failed".into()));
                }

                info!("Testing WebSocket endpoint...");
                if DnsChecker::check_websocket_endpoint(&config.network.domain).await? {
                    println!("{}", "WebSocket endpoint: OK".green());
                } else {
                    return Err(Error::Network("WebSocket endpoint test failed".into()));
                }
            }

            if !args.no_docker {
                info!("Checking Docker dependencies...");
                DockerService::check_dependencies().await?;
                println!("{}", "Docker dependencies: OK".green());
            }

            println!("{}", "Environment check completed successfully!".green());
            Ok(())
        }

        Commands::CreateAccount(args) => {
            let config = Config::load(config_path)?;
            info!("Creating account: {}", args.handle);

            let client = PdsClient::new(&config.network.domain);
            let account = client
                .create_account(args.handle, args.email, args.password)
                .await?;

            println!("{}", "Account created successfully!".green());
            println!("DID: {}", account.did);
            println!("Handle: {}", account.handle);
            Ok(())
        }

        Commands::Certs(args) => {
            if args.self_signed {
                info!("Generating self-signed certificates...");
                CertManager::generate_self_signed_ca("certs").await?;
                println!("{}", "Certificates generated successfully!".green());

                info!("Installing CA certificate...");
                CertManager::install_ca_cert("certs/root.crt").await?;
                println!("{}", "CA certificate installed successfully!".green());
            }
            Ok(())
        }

        Commands::DeployFeed(args) => {
            let config = Config::load(config_path)?;
            info!("Deploying feed generator...");

            // Add feed generator to compose
            let mut compose = ComposeConfig::load("docker-compose.yml")?;
            compose.add_feed_generator(&args.publisher_did);
            compose.save("docker-compose.yml")?;

            // Start feed generator
            let docker = DockerService::new("docker-compose.yml");
            docker
                .start_services(Some(&[String::from("feed-generator")]))
                .await?;

            // Publish feed
            let feed_gen = FeedGenerator::new(&config.network.domain, &args.publisher_did);
            let response = feed_gen.publish_feed().await?;

            println!("{}", "Feed generator deployed successfully!".green());
            println!("Feed URI: {}", response.uri);
            println!("Feed CID: {}", response.cid);
            Ok(())
        }

        Commands::DeployOzone(args) => {
            let config = Config::load(config_path)?;
            info!("Deploying Ozone service...");

            // Parse admin DIDs
            let admin_dids: Vec<String> = args
                .admin_dids
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();

            // Add Ozone to compose
            let mut compose = ComposeConfig::load("docker-compose.yml")?;
            compose.add_ozone(&args.server_did, &admin_dids);
            compose.save("docker-compose.yml")?;

            // Update Caddy configuration
            let caddy = CaddyConfig::new(&config.network.domain);
            caddy.save("config/caddy/Caddyfile")?;

            println!("{}", "Ozone service deployed successfully!".green());
            println!("Server DID: {}", args.server_did);
            println!("Admin DIDs: {}", args.admin_dids);
            println!("\nNext step: Configure admin using:");
            println!("  atc configure-ozone --handle <handle> --plc-sign-token <token>");
            Ok(())
        }

        Commands::ConfigureOzone(args) => {
            let config = Config::load(config_path)?;
            info!("Configuring Ozone admin settings...");

            // Create Ozone client with base URL
            let base_url = format!("https://ozone.{}", config.network.domain);
            let ozone = OzoneClient::new(&base_url);

            // Use provided Ozone URL or construct default
            let ozone_url = args.ozone_url.unwrap_or_else(|| base_url.clone());

            // Update DID doc with the PLC sign token
            let response = ozone
                .update_did_doc(&args.plc_sign_token, &args.handle, &ozone_url)
                .await?;

            println!("{}", "Ozone admin configured successfully!".green());
            println!("DID: {}", response.did);
            println!("Updated: {}", response.updated);
            println!("\nYou can now sign in to Ozone at:");
            println!("https://ozone.{}", config.network.domain);
            Ok(())
        }

        Commands::Status(args) => {
            let _config = Config::load(config_path)?;
            info!("Getting service status...");

            let docker = DockerService::new("docker-compose.yml");
            let status_manager = crate::status::StatusManager::new(docker);

            let system_status = status_manager.get_status(args.verbose).await?;
            status_manager.print_status(&system_status, args.verbose);

            Ok(())
        }

        Commands::Health(args) => {
            let config = Config::load(config_path)?;
            info!("Checking service health...");

            let checker = crate::health::HealthChecker::new(&config.network.domain);

            let services = args.services.unwrap_or_else(|| {
                vec![
                    "pds".to_string(),
                    "plc".to_string(),
                    "bgs".to_string(),
                    "appview".to_string(),
                    "social-app".to_string(),
                    "ozone".to_string(),
                    "feed-generator".to_string(),
                    "jetstream".to_string(),
                ]
            });

            for service in services {
                let status = checker.check_service(&service).await?;
                print_health_status(&status, args.verbose);
            }

            Ok(())
        }

        Commands::DeployJetstream(args) => {
            let config = Config::load(config_path)?;
            info!("Deploying Jetstream service...");

            // Update docker-compose with Jetstream service
            let mut compose = ComposeConfig::load("docker-compose.yml")?;
            compose.add_jetstream(args.reconnect_delay.unwrap_or(200));
            compose.save("docker-compose.yml")?;

            // Start Jetstream service
            let docker = DockerService::new("docker-compose.yml");
            docker
                .start_services(Some(&[String::from("jetstream")]))
                .await?;

            println!("{}", "Jetstream service deployed successfully!".green());
            println!(
                "Jetstream endpoint: wss://jetstream.{}",
                config.network.domain
            );
            Ok(())
        }

        Commands::Subscribe(args) => {
            let config = Config::load(config_path)?;
            info!("Subscribing to Jetstream collections...");

            let client = crate::jetstream::JetstreamClient::new(&config.network.domain);
            client.subscribe(&args.collections).await?;

            println!("{}", "Subscribed to collections successfully!".green());
            Ok(())
        }
    }
}

fn create_env_vars(config: &Config) -> std::collections::HashMap<String, String> {
    let mut vars = std::collections::HashMap::new();
    vars.insert("DOMAIN".to_string(), config.network.domain.clone());
    vars.insert(
        "BIND_ADDRESS".to_string(),
        config.network.bind_address.clone(),
    );
    vars.insert("USE_TLS".to_string(), config.network.use_tls.to_string());
    vars
}

fn print_health_status(status: &crate::health::HealthStatus, verbose: bool) {
    use crate::health::HealthState;

    let indicator = match status.status {
        HealthState::Healthy => "✓".green().to_string(),
        HealthState::Degraded => "!".yellow().to_string(),
        HealthState::Unhealthy => "✗".red().to_string(),
    };

    print!("{} {} ", indicator, status.service.bold());

    if verbose {
        println!();
        println!("  Latency: {}ms", status.latency_ms);
        if let Some(details) = &status.details {
            println!("  Details: {}", details);
        }
    } else {
        let state = match status.status {
            HealthState::Healthy => "✓".green().to_string(),
            HealthState::Degraded => "!!".yellow().to_string(),
            HealthState::Unhealthy => "✗".red().to_string(),
        };
        println!("- {}", state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::*;
    use assert_fs::prelude::*;
    use std::{env, path::PathBuf};
    use test_case::test_case;
    use wiremock::{matchers::{method, path, header}, Mock, MockServer, ResponseTemplate};

    struct TestContext {
        temp_dir: assert_fs::TempDir,
        config_path: assert_fs::fixture::ChildPath,
        _guard: DirectoryGuard,
    }

    struct DirectoryGuard {
        original: PathBuf,
    }

    impl DirectoryGuard {
        fn new(temp_dir: &std::path::Path) -> Self {
            let original = env::current_dir().expect("Failed to get current directory");
            env::set_current_dir(temp_dir).expect("Failed to set current directory");
            Self { original }
        }
    }

    impl Drop for DirectoryGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original);
        }
    }

    impl TestContext {
        fn new() -> Self {
            let temp_dir = assert_fs::TempDir::new().unwrap();
            let guard = DirectoryGuard::new(temp_dir.path());

            // Create base directories
            temp_dir.child("config").create_dir_all().unwrap();
            temp_dir.child("config/caddy").create_dir_all().unwrap();
            temp_dir.child("certs").create_dir_all().unwrap();
            temp_dir.child("data").create_dir_all().unwrap();

            let config_path = temp_dir.child("config.toml");

            Self {
                temp_dir,
                config_path,
                _guard: guard,
            }
        }

        fn get_path(&self, path: &str) -> assert_fs::fixture::ChildPath {
            self.temp_dir.child(path)
        }

        fn setup_initial_config(&self, domain: &str) -> Result<()> {
            // Create initial config
            let config = Config {
                network: crate::config::NetworkConfig {
                    domain: domain.to_string(),
                    ..Default::default()
                },
                storage: crate::config::StorageConfig::default(),
                email: crate::config::EmailConfig {
                    cert_email: "test@example.com".to_string(),
                    ..Default::default()
                },
            };
            config.save(&self.config_path)?;

            // Create compose file
            let mut compose = ComposeConfig::new();
            compose
                .add_caddy()
                .add_pds(domain)
                .add_plc()
                .add_bgs()
                .add_appview();
            compose.save(self.get_path("docker-compose.yml").path())?;

            // Create Caddyfile
            let caddy = CaddyConfig::new(domain);
            caddy.save(self.get_path("config/caddy/Caddyfile").path())?;

            // Generate and save secrets
            let secrets = Secrets::generate();
            secrets.save(self.get_path("config/secrets.toml").path())?;

            Ok(())
        }

        fn verify_files_exist(&self) -> bool {
            self.get_path("config/secrets.toml").exists() &&
            self.get_path("docker-compose.yml").exists() &&
            self.get_path("config/caddy/Caddyfile").exists() &&
            self.config_path.exists()
        }
    }

    #[tokio::test]
    async fn test_init_command() -> Result<()> {
        let ctx = TestContext::new();

        let cmd = Commands::Init(InitArgs {
            domain: "test.com".to_string(),
            cert_email: "admin@test.com".to_string(),
        });

        handle_command(cmd, &ctx.config_path).await?;

        assert!(ctx.verify_files_exist(), "Not all required files were created");

        let config = Config::load(&ctx.config_path)?;
        assert_eq!(config.network.domain, "test.com");
        assert_eq!(config.email.cert_email, "admin@test.com");

        Ok(())
    }

    #[test_case("test.com", "admin@test.com" ; "valid domain and email")]
    #[test_case("localhost", "admin@localhost" ; "localhost config")]
    fn test_config_init_variations(domain: &str, email: &str) -> Result<()> {
        let ctx = TestContext::new();

        let config = Config {
            network: crate::config::NetworkConfig {
                domain: domain.to_string(),
                ..Default::default()
            },
            email: crate::config::EmailConfig {
                cert_email: email.to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        config.save(&ctx.config_path)?;
        assert!(ctx.config_path.exists());

        let loaded = Config::load(&ctx.config_path)?;
        assert_eq!(loaded.network.domain, domain);
        assert_eq!(loaded.email.cert_email, email);

        Ok(())
    }

    #[tokio::test]
    async fn test_check_command() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        let cmd = Commands::Check(CheckArgs {
            no_dns: true,
            no_docker: true,
        });

        handle_command(cmd, &ctx.config_path).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_check_command_with_dns() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        // Need compose file for check
        ctx.get_path("docker-compose.yml")
            .write_str("version: '3.8'\nservices: {}").unwrap();

        let cmd = Commands::Check(CheckArgs {
            no_dns: false,
            no_docker: true,
        });

        match handle_command(cmd, &ctx.config_path).await {
            Ok(_) => Ok(()),
            Err(Error::Network(_)) => Ok(()), // Expected for test domain
            Err(e) => Err(e),
        }
    }

    #[tokio::test]
    async fn test_start_command() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        let cmd = Commands::Start(StartArgs {
            services: Some(vec!["pds".to_string(), "plc".to_string()]),
            no_deps: true,
        });

        match handle_command(cmd, &ctx.config_path).await {
            Ok(_) => Ok(()),
            Err(Error::Docker(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[tokio::test]
    async fn test_stop_command() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        let cmd = Commands::Stop(StopArgs { clean: true });

        match handle_command(cmd, &ctx.config_path).await {
            Ok(_) => Ok(()),
            Err(Error::Docker(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[tokio::test]
    async fn test_create_account() -> Result<()> {
        let mock_server = MockServer::start().await;
        let ctx = TestContext::new();

        let uri = mock_server.uri();
        let domain = uri.trim_start_matches("http://");
        ctx.setup_initial_config(domain)?;

        Mock::given(method("POST"))
            .and(path("/xrpc/com.atproto.server.createAccount"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "did": "did:plc:testuser123",
                "handle": "test.example.com"
            })))
            .mount(&mock_server)
            .await;

        let cmd = Commands::CreateAccount(CreateAccountArgs {
            handle: "test.example.com".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
        });

        handle_command(cmd, &ctx.config_path).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_deploy_feed() -> Result<()> {
        let mock_server = MockServer::start().await;
        let ctx = TestContext::new();

        let uri = mock_server.uri();
        let domain = uri.trim_start_matches("http://");
        ctx.setup_initial_config(domain)?;

        Mock::given(method("POST"))
            .and(path("/scripts/publishFeedGen.ts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uri": "at://did:plc:feed123/app.bsky.feed.generator/test-feed",
                "cid": "bafytest123"
            })))
            .mount(&mock_server)
            .await;

        let cmd = Commands::DeployFeed(DeployFeedArgs {
            publisher_did: "did:plc:feed123".to_string(),
        });

        handle_command(cmd, &ctx.config_path).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_deploy_ozone() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        let cmd = Commands::DeployOzone(DeployOzoneArgs {
            server_did: "did:plc:test123".to_string(),
            admin_dids: "did:plc:admin456".to_string(),
        });

        handle_command(cmd, &ctx.config_path).await?;

        let compose = ComposeConfig::load(ctx.get_path("docker-compose.yml").path())?;
        assert!(compose.services.contains_key("ozone"));

        let ozone = compose.services.get("ozone").unwrap();
        assert!(ozone
            .environment
            .as_ref()
            .unwrap()
            .iter()
            .any(|e| e.contains("did:plc:test123")));

        Ok(())
    }

    #[tokio::test]
    async fn test_configure_ozone() -> Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/ozone/updateDidDoc"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "did": "did:plc:test123",
                "updated": true
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let ctx = TestContext::new();
        let mock_server_uri = mock_server.uri();
        let domain = mock_server_uri.trim_start_matches("http://");
        ctx.setup_initial_config(domain)?;

        let cmd = Commands::ConfigureOzone(ConfigureOzoneArgs {
            handle: format!("admin.{}", domain),
            plc_sign_token: "test_token".to_string(),
            ozone_url: Some(format!("https://ozone.{}", domain)),
        });

        handle_command(cmd, &ctx.config_path).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_status_command() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        let cmd = Commands::Status(StatusArgs { verbose: true });

        match handle_command(cmd, &ctx.config_path).await {
            Ok(_) => Ok(()),
            Err(Error::Docker(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[tokio::test]
    async fn test_health_command() -> Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/_health"))
            .and(header("Accept", "application/json"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let ctx = TestContext::new();
        ctx.setup_initial_config(mock_server.uri().trim_start_matches("http://"))?;

        let cmd = Commands::Health(HealthArgs {
            services: Some(vec!["pds".to_string()]),
            verbose: true,
        });

        handle_command(cmd, &ctx.config_path).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_jetstream_commands() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        let deploy_cmd = Commands::DeployJetstream(DeployJetstreamArgs {
            reconnect_delay: Some(300),
        });

        match handle_command(deploy_cmd, &ctx.config_path).await {
            Ok(_) => Ok(()),
            Err(Error::Docker(_)) => Ok(()),
            Err(e) => Err(e),
        }?;

        let subscribe_cmd = Commands::Subscribe(SubscribeArgs {
            collections: vec!["app.bsky.feed.post".to_string()],
        });

        handle_command(subscribe_cmd, &ctx.config_path).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_certs_command() -> Result<()> {
        let ctx = TestContext::new();
        ctx.setup_initial_config("test.com")?;

        ctx.get_path("certs/root.crt").write_str("test certificate").unwrap();
        ctx.get_path("certs/root.key").write_str("test key").unwrap();

        let cmd = Commands::Certs(CertArgs { self_signed: false });
        handle_command(cmd, &ctx.config_path).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_error_cases() -> Result<()> {
        let ctx = TestContext::new();

        // Test starting without config
        let cmd = Commands::Start(StartArgs {
            services: None,
            no_deps: true,
        });
        assert!(matches!(
            handle_command(cmd, &ctx.config_path).await,
            Err(Error::Docker(_))
        ));

        // Test checking without config/files
        let cmd = Commands::Check(CheckArgs {
            no_dns: true,
            no_docker: true,
        });
        assert!(matches!(
            handle_command(cmd, &ctx.config_path).await,
            Err(Error::Config(_))
        ));

        Ok(())
    }
}
