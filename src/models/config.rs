use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdinConfig {
    pub storage_dir: Option<String>,
    pub snapshot: SnapshotConfig,
    pub restore: RestoreConfig,
    pub sync: SyncConfig,
    pub github: GitHubConfig,
    // `#[serde(default)]` keeps pre-platform config.yaml files loadable.
    #[serde(default)]
    pub platform: PlatformConfig,
}

impl Default for OdinConfig {
    fn default() -> Self {
        Self {
            storage_dir: None,
            snapshot: SnapshotConfig::default(),
            restore: RestoreConfig {
                package_managers: vec![
                    "winget".to_string(),
                    "choco".to_string(),
                    "scoop".to_string(),
                ],
                restore_user_environment: true,
                restore_path: true,
                restore_vscode_extensions: true,
                restore_git_config: true,
            },
            sync: SyncConfig {
                branch: "main".to_string(),
                remote: None,
            },
            github: GitHubConfig {
                repository_url: None,
                branch: "main".to_string(),
                token_key: None,
            },
            platform: PlatformConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SnapshotConfig {
    /// Maximum number of historical snapshots to keep. Oldest are pruned automatically.
    /// If unset, snapshots accumulate indefinitely.
    pub keep_last: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreConfig {
    pub package_managers: Vec<String>,
    pub restore_user_environment: bool,
    pub restore_path: bool,
    pub restore_vscode_extensions: bool,
    pub restore_git_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub branch: String,
    pub remote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub repository_url: Option<String>,
    pub branch: String,
    pub token_key: Option<String>,
}

/// Odin Platform connection. `url` is the platform origin; the API token itself
/// lives in the OS credential store under `token_key` (never in config.yaml).
/// When `upload_on_snapshot` is set, `odin snapshot` (and `odin watch --follow`)
/// push captured snapshots to the platform automatically.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformConfig {
    pub url: Option<String>,
    pub token_key: Option<String>,
    #[serde(default)]
    pub upload_on_snapshot: bool,
}
