use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of file/environment change
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WatchEventType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// A change in a file or system resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub change_type: WatchEventType,
    pub path: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub timestamp: String, // ISO 8601
}

/// A change in an environment variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvChange {
    pub name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub timestamp: String,
}

/// A change in PATH directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathChange {
    pub change_type: WatchEventType,
    pub directory: String,
    pub timestamp: String,
}

/// A package installation or removal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageChange {
    pub manager: String, // winget, chocolatey, scoop
    pub package_name: String,
    pub version: String,
    pub action: String, // install, remove, update
    pub timestamp: String,
}

/// A watcher event combining all types of changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WatcherEvent {
    File(FileChange),
    EnvVar(EnvChange),
    Path(PathChange),
    Package(PackageChange),
}

/// Current state snapshot for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherState {
    pub environment_vars: HashMap<String, String>,
    pub path_entries: Vec<String>,
    pub files: HashMap<String, String>, // path -> content hash
    pub installed_packages: HashMap<String, String>, // name -> version
    pub timestamp: String,
}

impl WatcherState {
    pub fn new() -> Self {
        WatcherState {
            environment_vars: HashMap::new(),
            path_entries: Vec::new(),
            files: HashMap::new(),
            installed_packages: HashMap::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Capture current system state
    pub fn capture() -> std::io::Result<Self> {
        let mut state = WatcherState::new();

        // Capture environment variables
        for (key, value) in std::env::vars() {
            state.environment_vars.insert(key, value);
        }

        // Capture PATH entries
        if let Ok(path_str) = std::env::var("PATH") {
            state.path_entries = path_str.split(';').map(String::from).collect();
        }

        Ok(state)
    }

    /// Compare with another state to find changes
    pub fn diff(&self, other: &WatcherState) -> Vec<WatcherEvent> {
        let mut events = Vec::new();

        // Find changed environment variables
        for (key, value) in &self.environment_vars {
            match other.environment_vars.get(key) {
                None => {
                    events.push(WatcherEvent::EnvVar(EnvChange {
                        name: key.clone(),
                        old_value: Some(value.clone()),
                        new_value: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    }));
                }
                Some(new_value) if new_value != value => {
                    events.push(WatcherEvent::EnvVar(EnvChange {
                        name: key.clone(),
                        old_value: Some(value.clone()),
                        new_value: Some(new_value.clone()),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    }));
                }
                _ => {}
            }
        }

        // Find new environment variables
        for (key, value) in &other.environment_vars {
            if !self.environment_vars.contains_key(key) {
                events.push(WatcherEvent::EnvVar(EnvChange {
                    name: key.clone(),
                    old_value: None,
                    new_value: Some(value.clone()),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }));
            }
        }

        // Find changed PATH entries
        let old_paths: std::collections::HashSet<_> = self.path_entries.iter().cloned().collect();
        let new_paths: std::collections::HashSet<_> = other.path_entries.iter().cloned().collect();

        for removed_path in old_paths.difference(&new_paths) {
            events.push(WatcherEvent::Path(PathChange {
                change_type: WatchEventType::Deleted,
                directory: removed_path.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            }));
        }

        for added_path in new_paths.difference(&old_paths) {
            events.push(WatcherEvent::Path(PathChange {
                change_type: WatchEventType::Created,
                directory: added_path.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            }));
        }

        events
    }
}

impl Default for WatcherState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_state_creation() {
        let state = WatcherState::new();
        assert!(!state.timestamp.is_empty());
    }

    #[test]
    fn test_diff_empty() {
        let state1 = WatcherState::new();
        let state2 = WatcherState::new();
        let diff = state1.diff(&state2);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_diff_env_var_added() {
        let mut state1 = WatcherState::new();
        let mut state2 = WatcherState::new();

        state2
            .environment_vars
            .insert("NEW_VAR".to_string(), "value".to_string());

        let diff = state1.diff(&state2);
        assert!(!diff.is_empty());
    }
}
