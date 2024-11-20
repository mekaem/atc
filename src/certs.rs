use crate::error::{Error, Result};
use std::{path::Path, process::Stdio};
use tokio::process::Command;
use tracing::{debug, instrument};

pub struct CertManager;

impl CertManager {
    #[instrument]
    pub async fn generate_self_signed_ca(
        cert_dir: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<()> {
        let cert_dir = cert_dir.as_ref();
        debug!("Generating self-signed CA certificate in {:?}", cert_dir);

        // Ensure cert directory exists
        tokio::fs::create_dir_all(cert_dir).await?;

        let root_key = cert_dir.join("root.key");
        let root_cert = cert_dir.join("root.crt");

        // Generate root key
        let status = Command::new("openssl")
            .arg("genrsa")
            .arg("-out")
            .arg(&root_key)
            .arg("2048")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .await
            .map_err(|e| Error::Cert(format!("Failed to generate root key: {}", e)))?;

        if !status.success() {
            return Err(Error::Cert("Failed to generate root key".into()));
        }

        // Generate root certificate
        let status = Command::new("openssl")
            .arg("req")
            .arg("-x509")
            .arg("-new")
            .arg("-nodes")
            .arg("-key")
            .arg(&root_key)
            .arg("-sha256")
            .arg("-days")
            .arg("1024")
            .arg("-out")
            .arg(&root_cert)
            .arg("-subj")
            .arg("/C=US/ST=State/L=City/O=Org/CN=Local Dev CA")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .await
            .map_err(|e| Error::Cert(format!("Failed to generate root certificate: {}", e)))?;

        if !status.success() {
            return Err(Error::Cert("Failed to generate root certificate".into()));
        }

        Ok(())
    }

    #[instrument]
    pub async fn install_ca_cert(cert_path: impl AsRef<Path> + std::fmt::Debug) -> Result<()> {
        let cert_path = cert_path.as_ref();
        debug!("Installing CA certificate from {:?}", cert_path);

        // Linux (Ubuntu/Debian)
        if Path::new("/usr/local/share/ca-certificates").exists() {
            let status = Command::new("sudo")
                .arg("cp")
                .arg(cert_path)
                .arg("/usr/local/share/ca-certificates/atc-root.crt")
                .status()
                .await
                .map_err(|e| Error::Cert(format!("Failed to copy certificate: {}", e)))?;

            if !status.success() {
                return Err(Error::Cert("Failed to copy certificate".into()));
            }

            let status = Command::new("sudo")
                .arg("update-ca-certificates")
                .status()
                .await
                .map_err(|e| Error::Cert(format!("Failed to update certificates: {}", e)))?;

            if !status.success() {
                return Err(Error::Cert("Failed to update certificates".into()));
            }
        }
        // macOS
        else if Path::new("/usr/local/etc/ca-certificates").exists() {
            let status = Command::new("sudo")
                .arg("security")
                .arg("add-trusted-cert")
                .arg("-d")
                .arg("-r")
                .arg("trustRoot")
                .arg("-k")
                .arg("/Library/Keychains/System.keychain")
                .arg(cert_path)
                .status()
                .await
                .map_err(|e| Error::Cert(format!("Failed to install certificate: {}", e)))?;

            if !status.success() {
                return Err(Error::Cert("Failed to install certificate".into()));
            }
        } else {
            return Err(Error::Cert("Unsupported operating system".into()));
        }

        Ok(())
    }

    #[instrument]
    pub async fn check_cert_exists(cert_dir: impl AsRef<Path> + std::fmt::Debug) -> Result<bool> {
        let cert_dir = cert_dir.as_ref();
        let root_key = cert_dir.join("root.key");
        let root_cert = cert_dir.join("root.crt");

        Ok(root_key.exists() && root_cert.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_generate_ca_cert() -> Result<()> {
        let temp_dir = tempdir()?;
        CertManager::generate_self_signed_ca(&temp_dir).await?;

        assert!(temp_dir.path().join("root.key").exists());
        assert!(temp_dir.path().join("root.crt").exists());
        Ok(())
    }

    #[tokio::test]
    async fn test_check_cert_exists() -> Result<()> {
        let temp_dir = tempdir()?;

        // Should return false when no certs exist
        assert!(!CertManager::check_cert_exists(&temp_dir).await?);

        // Generate certs
        CertManager::generate_self_signed_ca(&temp_dir).await?;

        // Should return true after generation
        assert!(CertManager::check_cert_exists(&temp_dir).await?);
        Ok(())
    }
}
