use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::integrations::archive;
use crate::models::history::SnapshotMetadata;
use crate::models::machine::MachineSnapshot;
use crate::services::history_service::HistoryService;
use crate::services::storage::{self, SnapshotStore};

pub struct ArchiveService {
    odin_dir: PathBuf,
}

impl ArchiveService {
    pub fn new(odin_dir: PathBuf) -> Self {
        Self { odin_dir }
    }

    /// Bundle a historical snapshot directory (~/.odin/history/<id>/) into a .tar.gz file.
    pub fn export(&self, snapshot_id: &str, output: &Path) -> Result<PathBuf> {
        let history_service = HistoryService::new(self.odin_dir.clone());
        let resolved_id = history_service.resolve(snapshot_id)?;
        let snapshot_dir = self.odin_dir.join("history").join(&resolved_id);
        if !snapshot_dir.is_dir() {
            anyhow::bail!(
                "no archived files for snapshot {} at {}",
                resolved_id,
                snapshot_dir.display()
            );
        }
        archive::create_tarball(&snapshot_dir, output)?;
        Ok(snapshot_dir)
    }

    /// Extract a previously exported snapshot bundle into a new historical snapshot id and
    /// register it in the history index.
    pub async fn import(&self, archive_path: &Path) -> Result<SnapshotMetadata> {
        let new_id = uuid::Uuid::new_v4().to_string();
        let target_dir = self.odin_dir.join("history").join(&new_id);
        archive::extract_tarball(archive_path, &target_dir)?;

        let machine_path = target_dir.join(storage::MACHINE);
        if !machine_path.exists() {
            anyhow::bail!(
                "archive did not contain {}; refusing to register a malformed snapshot",
                storage::MACHINE
            );
        }
        let store = SnapshotStore::new(target_dir.clone());
        let machine: MachineSnapshot = store
            .read_machine()
            .await
            .with_context(|| format!("reading machine snapshot from {}", target_dir.display()))?;
        let packages = store.read_packages().await?;

        let metadata = SnapshotMetadata {
            id: new_id,
            timestamp: machine.captured_at.to_rfc3339(),
            hostname: machine.hostname.clone(),
            os_version: machine.os_version.clone(),
            total_packages: packages.packages.len(),
            tag: Some(format!(
                "imported-{}",
                machine.captured_at.format("%Y%m%d-%H%M%S")
            )),
        };

        HistoryService::new(self.odin_dir.clone()).register_snapshot(metadata.clone())?;
        Ok(metadata)
    }

    /// Tar+gzip an arbitrary directory.
    pub fn create(&self, input_dir: &Path, output: &Path) -> Result<()> {
        archive::create_tarball(input_dir, output)
    }

    /// Extract a tar.gz archive into an arbitrary output directory.
    pub fn extract(&self, input: &Path, output_dir: &Path) -> Result<()> {
        archive::extract_tarball(input, output_dir)
    }
}
