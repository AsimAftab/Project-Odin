use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use crate::models::watcher::{WatcherEvent, WatcherState};

pub struct WatcherService {
    record_path: Option<PathBuf>,
}

impl WatcherService {
    pub fn new(record_path: Option<PathBuf>) -> Self {
        Self { record_path }
    }

    pub fn capture(&self) -> Result<WatcherState> {
        WatcherState::capture().context("capturing environment state")
    }

    pub async fn sleep(&self, interval_secs: u64) {
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }

    pub async fn record(&self, events: &[WatcherEvent]) -> Result<()> {
        let Some(path) = &self.record_path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .with_context(|| format!("opening {} for append", path.display()))?;
        for event in events {
            let line = serde_json::to_string(event)?;
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
        Ok(())
    }

    pub fn record_path(&self) -> Option<&Path> {
        self.record_path.as_deref()
    }
}
