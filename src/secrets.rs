use crate::error::{Error, Result};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct Secrets {
    pub pds_jwt_secret: String,
    pub pds_admin_password: String,
    pub pds_plc_rotation_key: String,
}

impl Secrets {
    #[instrument]
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();

        Self {
            pds_jwt_secret: generate_secure_string(&mut rng, 32),
            pds_admin_password: generate_secure_string(&mut rng, 16),
            pds_plc_rotation_key: generate_base32_key(&mut rng),
        }
    }

    #[instrument(skip(path))]
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize secrets: {}", e)))?;

        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(())
    }

    #[instrument(skip(path))]
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse secrets: {}", e)))
    }

    pub fn as_env_vars(&self) -> Vec<(String, String)> {
        vec![
            ("PDS_JWT_SECRET".into(), self.pds_jwt_secret.clone()),
            ("PDS_ADMIN_PASSWORD".into(), self.pds_admin_password.clone()),
            (
                "PDS_PLC_ROTATION_KEY_K256".into(),
                self.pds_plc_rotation_key.clone(),
            ),
        ]
    }
}

fn generate_secure_string(rng: &mut impl Rng, len: usize) -> String {
    rng.sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn generate_base32_key(rng: &mut impl Rng) -> String {
    const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    (0..52)
        .map(|_| {
            let idx = rng.gen_range(0..BASE32_ALPHABET.len());
            BASE32_ALPHABET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;

    #[test]
    fn test_generate_secrets() {
        let secrets = Secrets::generate();

        // Check JWT secret
        assert_eq!(secrets.pds_jwt_secret.len(), 32);
        assert!(secrets
            .pds_jwt_secret
            .chars()
            .all(|c| c.is_ascii_alphanumeric()));

        // Check admin password
        assert_eq!(secrets.pds_admin_password.len(), 16);
        assert!(secrets
            .pds_admin_password
            .chars()
            .all(|c| c.is_ascii_alphanumeric()));

        // Check PLC rotation key
        assert_eq!(secrets.pds_plc_rotation_key.len(), 52);
        assert!(secrets
            .pds_plc_rotation_key
            .chars()
            .all(|c| "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".contains(c)));
    }

    #[test]
    fn test_secrets_roundtrip() -> Result<()> {
        let temp = assert_fs::TempDir::new().unwrap();
        let secrets_path = temp.child("secrets.toml");

        let secrets = Secrets::generate();
        secrets.save(&secrets_path)?;

        let loaded = Secrets::load(&secrets_path)?;

        assert_eq!(secrets.pds_jwt_secret, loaded.pds_jwt_secret);
        assert_eq!(secrets.pds_admin_password, loaded.pds_admin_password);
        assert_eq!(secrets.pds_plc_rotation_key, loaded.pds_plc_rotation_key);

        Ok(())
    }

    #[test]
    fn test_env_vars() {
        let secrets = Secrets::generate();
        let env_vars = secrets.as_env_vars();

        assert!(env_vars.iter().any(|(k, _)| k == "PDS_JWT_SECRET"));
        assert!(env_vars.iter().any(|(k, _)| k == "PDS_ADMIN_PASSWORD"));
        assert!(env_vars
            .iter()
            .any(|(k, _)| k == "PDS_PLC_ROTATION_KEY_K256"));

        for (_, v) in env_vars {
            assert!(!v.is_empty());
        }
    }
}
