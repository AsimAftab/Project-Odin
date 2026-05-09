use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;

use crate::core::errors::OdinError;
use crate::models::environment::EnvironmentSnapshot;
use crate::models::git::GitConfigSnapshot;
use crate::models::machine::MachineSnapshot;
use crate::models::package::PackageSnapshot;
use crate::models::snapshot::{LockedFile, OdinLock};
use crate::models::vscode::VsCodeExtensionsSnapshot;
use crate::utils::{checksum, fs};

pub const MACHINE: &str = "machine.json";
pub const ENV: &str = "env.json";
pub const PACKAGES: &str = "packages.json";
pub const VSCODE: &str = "vscode_extensions.json";
pub const GIT: &str = "git_config.json";
pub const LOCK: &str = "odin.lock";

#[derive(Debug, Clone)]
pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub async fn ensure(&self) -> Result<()> {
        fs::ensure_dir(&self.root).await
    }

    pub async fn write_snapshot(
        &self,
        machine: &MachineSnapshot,
        environment: &EnvironmentSnapshot,
        packages: &PackageSnapshot,
        vscode: &VsCodeExtensionsSnapshot,
        git: &GitConfigSnapshot,
    ) -> Result<OdinLock> {
        self.ensure().await?;
        fs::write_json(&self.path(MACHINE), machine).await?;
        fs::write_json(&self.path(ENV), environment).await?;
        fs::write_json(&self.path(PACKAGES), packages).await?;
        fs::write_json(&self.path(VSCODE), vscode).await?;
        fs::write_json(&self.path(GIT), git).await?;

        let lock = self.lock_for(machine.snapshot_id).await?;
        fs::write_json(&self.path(LOCK), &lock).await?;
        Ok(lock)
    }

    pub async fn lock_for(&self, snapshot_id: uuid::Uuid) -> Result<OdinLock> {
        let names = [MACHINE, ENV, PACKAGES, VSCODE, GIT];
        let mut files = Vec::new();
        for name in names {
            let path = self.path(name);
            if path.exists() {
                files.push(LockedFile {
                    path: name.to_string(),
                    sha256: checksum::sha256_file(&path).await?,
                });
            }
        }
        Ok(OdinLock {
            schema_version: 1,
            generated_at: Utc::now(),
            snapshot_id,
            files,
        })
    }

    pub async fn read_machine(&self) -> Result<MachineSnapshot> {
        self.ensure_snapshot_file(MACHINE)?;
        fs::read_json(&self.path(MACHINE)).await
    }

    pub async fn read_environment(&self) -> Result<EnvironmentSnapshot> {
        self.ensure_snapshot_file(ENV)?;
        fs::read_json(&self.path(ENV)).await
    }

    pub async fn read_packages(&self) -> Result<PackageSnapshot> {
        self.ensure_snapshot_file(PACKAGES)?;
        fs::read_json(&self.path(PACKAGES)).await
    }

    pub async fn read_vscode(&self) -> Result<VsCodeExtensionsSnapshot> {
        self.ensure_snapshot_file(VSCODE)?;
        fs::read_json(&self.path(VSCODE)).await
    }

    pub async fn read_git(&self) -> Result<GitConfigSnapshot> {
        self.ensure_snapshot_file(GIT)?;
        fs::read_json(&self.path(GIT)).await
    }

    fn ensure_snapshot_file(&self, name: &str) -> Result<()> {
        if self.path(name).exists() {
            Ok(())
        } else {
            Err(OdinError::MissingSnapshot(name.to_string()).into())
        }
    }
}
