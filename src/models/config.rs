use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdinConfig {
    pub storage_dir: Option<String>,
    pub restore: RestoreConfig,
    pub sync: SyncConfig,
    pub github: GitHubConfig,
}

impl Default for OdinConfig {
    fn default() -> Self {
        Self {
            storage_dir: None,
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
        }
    }
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
