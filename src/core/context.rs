use std::path::PathBuf;

use anyhow::Result;

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

fn load_config(odin_dir: &std::path::Path) -> Result<OdinConfig> {
    let config_path = odin_dir.join("config.yaml");
    if !config_path.exists() {
        return Ok(OdinConfig::default());
    }
    let content = std::fs::read_to_string(config_path)?;
    Ok(serde_yaml::from_str(&content)?)
}
