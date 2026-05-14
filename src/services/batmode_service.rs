use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::models::batmode::{BatmodeConfig, BatmodeEntry};

const FILE_NAME: &str = "batmode.yaml";

pub struct BatmodeService {
    config_path: PathBuf,
}

impl BatmodeService {
    pub fn new(odin_dir: &Path) -> Self {
        Self {
            config_path: odin_dir.join(FILE_NAME),
        }
    }

    pub fn load(&self) -> Result<BatmodeConfig> {
        if !self.config_path.exists() {
            return Ok(BatmodeConfig::default());
        }
        let content = std::fs::read_to_string(&self.config_path)
            .with_context(|| format!("reading {}", self.config_path.display()))?;
        if content.trim().is_empty() {
            return Ok(BatmodeConfig::default());
        }
        Ok(serde_yaml::from_str(&content)?)
    }

    pub fn save(&self, config: &BatmodeConfig) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml::to_string(config)?;
        std::fs::write(&self.config_path, yaml)
            .with_context(|| format!("writing {}", self.config_path.display()))?;
        Ok(())
    }

    pub fn add(&self, profile: &str, entry: BatmodeEntry) -> Result<()> {
        let mut config = self.load()?;
        config
            .profiles
            .entry(profile.to_string())
            .or_default()
            .push(entry);
        self.save(&config)
    }

    pub fn remove_entry(&self, profile: &str, index: usize) -> Result<BatmodeEntry> {
        let mut config = self.load()?;
        let entries = config
            .profiles
            .get_mut(profile)
            .ok_or_else(|| anyhow::anyhow!("profile '{}' not found", profile))?;
        if index >= entries.len() {
            anyhow::bail!(
                "index {} out of range; profile '{}' has {} entries",
                index,
                profile,
                entries.len()
            );
        }
        let removed = entries.remove(index);
        if entries.is_empty() {
            config.profiles.remove(profile);
        }
        self.save(&config)?;
        Ok(removed)
    }

    pub fn remove_profile(&self, profile: &str) -> Result<()> {
        let mut config = self.load()?;
        if config.profiles.remove(profile).is_none() {
            anyhow::bail!("profile '{}' not found", profile);
        }
        self.save(&config)
    }

    pub fn launch(&self, profile: &str) -> Result<LaunchSummary> {
        let config = self.load()?;
        let entries = config
            .profiles
            .get(profile)
            .ok_or_else(|| anyhow::anyhow!("profile '{}' not found", profile))?;
        let total = entries.len();
        let mut launched = 0usize;
        let mut failures = Vec::new();
        for entry in entries {
            match std::process::Command::new(&entry.path)
                .args(&entry.args)
                .spawn()
            {
                Ok(_) => {
                    launched += 1;
                }
                Err(err) => {
                    failures.push(format!("{}: {}", entry.path, err));
                }
            }
        }
        Ok(LaunchSummary {
            launched,
            total,
            failures,
        })
    }
}

pub struct LaunchSummary {
    pub launched: usize,
    pub total: usize,
    pub failures: Vec<String>,
}
