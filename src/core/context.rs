use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::config::OdinConfig;
use crate::utils::paths;

#[derive(Debug, Clone)]
pub struct AppContext {
    odin_dir: PathBuf,
    config: OdinConfig,
}

impl AppContext {
    pub fn new(odin_dir: Option<PathBuf>) -> Result<Self> {
        let odin_dir = match odin_dir {
            Some(path) => path,
            None => paths::default_odin_dir()?,
        };
        let config = load_config(&odin_dir)?;
        Ok(Self { odin_dir, config })
    }

    pub fn odin_dir(&self) -> &PathBuf {
        &self.odin_dir
    }

    pub fn config(&self) -> &OdinConfig {
        &self.config
    }
}

fn load_config(odin_dir: &Path) -> Result<OdinConfig> {
    let config_path = odin_dir.join("config.yaml");
    if !config_path.exists() {
        ensure_workspace(odin_dir).with_context(|| {
            format!(
                "failed to initialize Odin workspace at {}",
                odin_dir.display()
            )
        })?;
        return Ok(OdinConfig::default());
    }
    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    Ok(serde_yaml::from_str(&content)?)
}

// First-run bootstrap: creates ~/.odin, the standard subdirs, and a default
// config.yaml. Runs implicitly so winget portable installs / fresh checkouts
// don't require an explicit `odin init`. Stays silent — `odin init` remains
// for explicit validation, dep checks, and PATH prompts.
fn ensure_workspace(odin_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(odin_dir)?;
    for sub in ["snapshots", "logs", "cache", "temp", "plugins"] {
        std::fs::create_dir_all(odin_dir.join(sub))?;
    }
    let config_path = odin_dir.join("config.yaml");
    if !config_path.exists() {
        let yaml = serde_yaml::to_string(&OdinConfig::default())?;
        std::fs::write(&config_path, yaml)?;
    }
    Ok(())
}
