use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::asgard::profile::{Profile, ProfileSummary};
use crate::asgard::state::AsgardState;
use crate::utils::fs;

pub const PROFILE_FILE: &str = "profile.yaml";
pub const STATE_FILE: &str = ".state.json";

#[derive(Debug, Clone)]
pub struct AsgardStore {
    root: PathBuf,
}

impl AsgardStore {
    pub fn new(odin_dir: &Path) -> Self {
        Self {
            root: odin_dir.join("asgard"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn profile_path(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join(PROFILE_FILE)
    }

    pub fn state_path(&self) -> PathBuf {
        self.root.join(STATE_FILE)
    }

    pub async fn ensure(&self) -> Result<()> {
        fs::ensure_dir(&self.root).await
    }

    pub async fn list(&self) -> Result<Vec<String>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        let mut rd = tokio::fs::read_dir(&self.root)
            .await
            .with_context(|| format!("failed to read {}", self.root.display()))?;
        while let Some(entry) = rd.next_entry().await? {
            let ft = entry.file_type().await?;
            if !ft.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if entry.path().join(PROFILE_FILE).exists() {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    pub async fn list_summaries(&self) -> Result<Vec<ProfileSummary>> {
        let mut out = Vec::new();
        for name in self.list().await? {
            match self.load(&name).await {
                Ok(p) => out.push((&p).into()),
                Err(_) => continue,
            }
        }
        Ok(out)
    }

    pub fn exists(&self, name: &str) -> bool {
        self.profile_path(name).exists()
    }

    pub async fn load(&self, name: &str) -> Result<Profile> {
        let path = self.profile_path(name);
        if !path.exists() {
            return Err(anyhow!("profile `{name}` not found"));
        }
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        let profile: Profile = serde_yaml::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(profile)
    }

    pub async fn save(&self, profile: &Profile) -> Result<()> {
        self.ensure().await?;
        fs::ensure_dir(&self.profile_dir(&profile.name)).await?;
        let yaml = serde_yaml::to_string(profile)?;
        fs::write_text(&self.profile_path(&profile.name), &yaml).await
    }

    pub async fn delete(&self, name: &str) -> Result<()> {
        let dir = self.profile_dir(name);
        if !dir.exists() {
            return Err(anyhow!("profile `{name}` not found"));
        }
        tokio::fs::remove_dir_all(&dir)
            .await
            .with_context(|| format!("failed to remove {}", dir.display()))?;
        Ok(())
    }

    pub async fn load_state(&self) -> Result<AsgardState> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(AsgardState::default());
        }
        fs::read_json(&path).await
    }

    pub async fn save_state(&self, state: &AsgardState) -> Result<()> {
        self.ensure().await?;
        fs::write_json(&self.state_path(), state).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asgard::profile::{StartupApp, WindowState};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn sample(name: &str) -> Profile {
        Profile {
            name: name.into(),
            description: "test".into(),
            env: BTreeMap::new(),
            startup_apps: vec![StartupApp {
                name: "n".into(),
                command: "notepad".into(),
                args: vec![],
                cwd: None,
                window: WindowState::Normal,
            }],
            vscode_workspace: None,
            browser_urls: vec![],
        }
    }

    #[tokio::test]
    async fn list_save_load_delete_round_trip() {
        let tmp = tempdir().unwrap();
        let store = AsgardStore::new(tmp.path());
        assert!(store.list().await.unwrap().is_empty());

        store.save(&sample("alpha")).await.unwrap();
        store.save(&sample("beta")).await.unwrap();

        let names = store.list().await.unwrap();
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);

        let loaded = store.load("alpha").await.unwrap();
        assert_eq!(loaded.name, "alpha");
        assert_eq!(loaded.startup_apps.len(), 1);

        let summaries = store.list_summaries().await.unwrap();
        assert_eq!(summaries.len(), 2);

        store.delete("alpha").await.unwrap();
        assert_eq!(store.list().await.unwrap(), vec!["beta".to_string()]);
        assert!(store.load("alpha").await.is_err());
    }

    #[tokio::test]
    async fn state_round_trip() {
        let tmp = tempdir().unwrap();
        let store = AsgardStore::new(tmp.path());
        let mut s = store.load_state().await.unwrap();
        assert!(s.active_profile.is_none());
        s.record_activation("alpha", chrono::Utc::now());
        store.save_state(&s).await.unwrap();
        let back = store.load_state().await.unwrap();
        assert_eq!(back.active_profile.as_deref(), Some("alpha"));
        assert_eq!(back.recent.len(), 1);
    }

    #[tokio::test]
    async fn list_ignores_dotdirs_and_state_file() {
        let tmp = tempdir().unwrap();
        let store = AsgardStore::new(tmp.path());
        store.save(&sample("real")).await.unwrap();
        let mut s = AsgardState::default();
        s.record_activation("real", chrono::Utc::now());
        store.save_state(&s).await.unwrap();
        // create a stray dotdir
        tokio::fs::create_dir_all(store.root().join(".hidden"))
            .await
            .unwrap();
        // create a dir with no profile.yaml — should be ignored
        tokio::fs::create_dir_all(store.root().join("orphan"))
            .await
            .unwrap();
        let names = store.list().await.unwrap();
        assert_eq!(names, vec!["real".to_string()]);
    }
}
