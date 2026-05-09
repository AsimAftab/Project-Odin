use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::config::OdinConfig;
use crate::utils::fs;

#[derive(Debug, Clone)]
pub struct ConfigService {
    root: PathBuf,
}

impl ConfigService {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn path(&self) -> PathBuf {
        self.root.join("config.yaml")
    }

    pub async fn load(&self) -> Result<OdinConfig> {
        let path = self.path();
        if !path.exists() {
            return Ok(OdinConfig::default());
        }
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub async fn save(&self, config: &OdinConfig) -> Result<()> {
        let content = serde_yaml::to_string(config)?;
        fs::write_text(&self.path(), &content).await
    }

    pub async fn init(&self, force: bool) -> Result<PathBuf> {
        fs::ensure_dir(&self.root).await?;
        let path = self.path();
        if force || !Path::new(&path).exists() {
            self.save(&OdinConfig::default()).await?;
        }
        Ok(path)
    }
}
