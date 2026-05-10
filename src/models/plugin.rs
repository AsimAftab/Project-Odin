use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Plugin manifest metadata (from plugin.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,

    /// Main entry point (executable path or script)
    pub executable: String,

    /// Minimum Odin version required
    #[serde(default)]
    pub min_odin_version: Option<String>,

    /// Commands provided by this plugin
    #[serde(default)]
    pub commands: Vec<PluginCommand>,

    /// Environment variables plugin needs/provides
    #[serde(default)]
    pub env_vars: Vec<String>,

    /// Plugin dependencies (other plugins)
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// Plugin hooks (pre-snapshot, post-restore, etc.)
    #[serde(default)]
    pub hooks: Vec<String>,
}

/// Command provided by plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    pub name: String,
    pub description: String,

    #[serde(default)]
    pub args: Vec<PluginArg>,

    #[serde(default)]
    pub examples: Vec<String>,
}

/// Argument for plugin command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginArg {
    pub name: String,
    pub description: String,

    #[serde(default)]
    pub required: bool,

    pub arg_type: String, // "string", "bool", "int", "path"
}

/// Information about an installed plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub install_path: PathBuf,
    pub installed_at: String, // ISO 8601
    pub enabled: bool,
    pub checksum: Option<String>, // SHA-256 for verification
}

impl InstalledPlugin {
    pub fn new(manifest: PluginManifest, install_path: PathBuf) -> Self {
        InstalledPlugin {
            manifest,
            install_path,
            installed_at: chrono::Utc::now().to_rfc3339(),
            enabled: true,
            checksum: None,
        }
    }
}

/// Available plugin from registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub download_url: String,
    pub checksum: String, // SHA-256
    pub min_odin_version: Option<String>,
    pub published_at: String,
    pub downloads: u32,
}

/// Plugin search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSearchResult {
    pub results: Vec<PluginRegistry>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

/// Plugin execution context
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub odin_dir: PathBuf,
    pub plugin_name: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Plugin execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl PluginResult {
    pub fn success(stdout: String) -> Self {
        PluginResult {
            success: true,
            stdout,
            stderr: String::new(),
            exit_code: 0,
        }
    }

    pub fn failure(stderr: String, exit_code: i32) -> Self {
        PluginResult {
            success: false,
            stdout: String::new(),
            stderr,
            exit_code,
        }
    }
}

/// Plugin registry index (local cache)
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginIndex {
    pub plugins: Vec<PluginRegistry>,
    pub last_updated: String,
}

impl PluginIndex {
    pub fn new() -> Self {
        PluginIndex {
            plugins: Vec::new(),
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn search(&self, query: &str) -> Vec<&PluginRegistry> {
        self.plugins
            .iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query.to_lowercase())
                    || p.description.to_lowercase().contains(&query.to_lowercase())
            })
            .collect()
    }

    pub fn find(&self, name: &str) -> Option<&PluginRegistry> {
        self.plugins.iter().find(|p| p.name == name)
    }
}

impl Default for PluginIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialization() {
        let manifest = PluginManifest {
            name: "docker-tools".to_string(),
            version: "1.0.0".to_string(),
            description: "Docker management tools".to_string(),
            author: "Copilot".to_string(),
            license: Some("MIT".to_string()),
            repository: None,
            homepage: None,
            executable: "docker-tools.exe".to_string(),
            min_odin_version: Some("0.1.0".to_string()),
            commands: vec![PluginCommand {
                name: "list-images".to_string(),
                description: "List Docker images".to_string(),
                args: vec![],
                examples: vec!["odin docker-tools list-images".to_string()],
            }],
            env_vars: vec![],
            dependencies: vec![],
            hooks: vec![],
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest.name, deserialized.name);
    }

    #[test]
    fn test_plugin_search() {
        let mut index = PluginIndex::new();
        index.plugins.push(PluginRegistry {
            name: "docker-tools".to_string(),
            version: "1.0.0".to_string(),
            description: "Docker management".to_string(),
            author: "Copilot".to_string(),
            download_url: "https://...".to_string(),
            checksum: "abc123".to_string(),
            min_odin_version: None,
            published_at: chrono::Utc::now().to_rfc3339(),
            downloads: 100,
        });

        let results = index.search("docker");
        assert_eq!(results.len(), 1);
    }
}
