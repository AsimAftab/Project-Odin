use anyhow::{Context, Result};

const SERVICE: &str = "odin-cli";

#[derive(Debug, Clone)]
pub struct SecretService;

impl SecretService {
    pub fn token_key(repository_url: &str) -> String {
        format!("github:{}", repository_url.trim().to_ascii_lowercase())
    }

    /// Keyring key for an Odin Platform API token, scoped by platform URL.
    pub fn platform_token_key(url: &str) -> String {
        format!("odin-platform:{}", url.trim().to_ascii_lowercase())
    }

    pub fn set_token(key: &str, token: &str) -> Result<()> {
        keyring::Entry::new(SERVICE, key)
            .context("failed to open OS credential store")?
            .set_password(token)
            .context("failed to store token in OS credential store")
    }

    pub fn get_token(key: &str) -> Result<String> {
        keyring::Entry::new(SERVICE, key)
            .context("failed to open OS credential store")?
            .get_password()
            .context("token was not found in OS credential store")
    }

    /// Removes a token from the OS credential store. Missing entries are not an
    /// error (logout is idempotent).
    pub fn delete_token(key: &str) -> Result<()> {
        let entry =
            keyring::Entry::new(SERVICE, key).context("failed to open OS credential store")?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::Error::new(e).context("failed to delete token")),
        }
    }
}
