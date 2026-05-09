use anyhow::{Context, Result};

const SERVICE: &str = "odin-cli";

#[derive(Debug, Clone)]
pub struct SecretService;

impl SecretService {
    pub fn token_key(repository_url: &str) -> String {
        format!("github:{}", repository_url.trim().to_ascii_lowercase())
    }

    pub fn set_token(key: &str, token: &str) -> Result<()> {
        keyring::Entry::new(SERVICE, key)
            .context("failed to open OS credential store")?
            .set_password(token)
            .context("failed to store GitHub token in OS credential store")
    }

    pub fn get_token(key: &str) -> Result<String> {
        keyring::Entry::new(SERVICE, key)
            .context("failed to open OS credential store")?
            .get_password()
            .context("GitHub token was not found in OS credential store")
    }
}
