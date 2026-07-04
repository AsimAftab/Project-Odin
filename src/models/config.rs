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
                // Every manager Odin captures, so the default restores everything.
                // Trim this list to restore only a subset. Aliases are accepted
                // (e.g. "choco" == "chocolatey"); see `source_enabled` in
                // services/restore_service.rs.
                package_managers: vec![
                    "winget".to_string(),
                    "chocolatey".to_string(),
                    "scoop".to_string(),
                    "npm".to_string(),
                    "pip".to_string(),
                    "cargo".to_string(),
                    "pipx".to_string(),
                    "pnpm".to_string(),
                    "yarn".to_string(),
                    "dotnet".to_string(),
                    "go".to_string(),
                    "uv".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_survives_yaml_round_trip() {
        let mut original = OdinConfig::default();
        original.platform.url = Some("https://odin.example.com".to_string());
        original.platform.token_key = Some("odin-platform:https://odin.example.com".to_string());
        original.platform.upload_on_snapshot = true;

        let yaml = serde_yaml::to_string(&original).unwrap();
        let restored: OdinConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(restored.platform.url, original.platform.url);
        assert!(restored.platform.upload_on_snapshot);
        assert_eq!(
            restored.restore.package_managers,
            original.restore.package_managers
        );
        assert!(restored.restore.restore_git_config);
    }

    #[test]
    fn pre_platform_config_still_loads() {
        // A config.yaml written before the `platform` block existed must load
        // (serde default fills it in), and before the new managers were added.
        let yaml = r#"
storage_dir: null
snapshot:
  keep_last: 5
restore:
  package_managers: ["winget"]
  restore_user_environment: true
  restore_path: true
  restore_vscode_extensions: true
  restore_git_config: true
sync:
  branch: main
  remote: null
github:
  repository_url: null
  branch: main
  token_key: null
"#;
        let cfg: OdinConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.restore.package_managers, vec!["winget".to_string()]);
        // Missing platform block defaults cleanly.
        assert_eq!(cfg.platform.url, None);
        assert!(!cfg.platform.upload_on_snapshot);
    }
}
