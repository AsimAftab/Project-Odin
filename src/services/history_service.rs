use crate::models::history::*;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Service for managing snapshot history and rollbacks
pub struct HistoryService {
    odin_dir: PathBuf,
}

impl HistoryService {
    pub fn new(odin_dir: impl AsRef<Path>) -> Self {
        HistoryService {
            odin_dir: odin_dir.as_ref().to_path_buf(),
        }
    }

    pub fn get_history(&self) -> Result<Vec<HistoryEntry>> {
        let history_file = self.odin_dir.join(".history");

        if !history_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&history_file)?;

        let index: HistoryIndex = serde_json::from_str(&content)?;

        // Convert snapshots to history entries with changes
        let mut entries = Vec::new();
        for (i, metadata) in index.snapshots.iter().enumerate() {
            let changes = if i + 1 < index.snapshots.len() {
                let next_snapshot = &index.snapshots[i + 1];
                self.get_snapshot_changes(&metadata.id, &next_snapshot.id)?
            } else {
                Vec::new() // No changes for the oldest snapshot
            };

            let summary = DiffSummary::from_changes(&changes);
            entries.push(HistoryEntry {
                metadata: metadata.clone(),
                changes,
                summary,
            });
        }

        Ok(entries)
    }

    pub fn compare_snapshots(&self, from_id: &str, to_id: &str) -> Result<SnapshotDiff> {
        let history_file = self.odin_dir.join(".history");

        if !history_file.exists() {
            return Err(anyhow::anyhow!("No history found"));
        }

        let content = fs::read_to_string(&history_file)?;

        let index: HistoryIndex = serde_json::from_str(&content)?;

        // Check if diff is cached
        if let Some(cached_diff) = index.get_diff(from_id, to_id) {
            return Ok(cached_diff.clone());
        }

        // If not cached, compute it
        let changes = self.compute_diff(from_id, to_id)?;
        let summary = DiffSummary::from_changes(&changes);

        Ok(SnapshotDiff {
            from_snapshot: from_id.to_string(),
            to_snapshot: to_id.to_string(),
            changes,
            summary,
        })
    }

    pub fn get_snapshot_changes(
        &self,
        snapshot_id: &str,
        previous_id: &str,
    ) -> Result<Vec<EnvironmentChange>> {
        self.compute_diff(previous_id, snapshot_id)
    }

    /// Resolve a string that may be a snapshot ID or a tag back to a snapshot ID.
    /// IDs match first; tags are searched only if no ID match is found.
    pub fn resolve(&self, id_or_tag: &str) -> Result<String> {
        let history_file = self.odin_dir.join(".history");
        if !history_file.exists() {
            anyhow::bail!("no snapshot history; run `odin snapshot` first");
        }
        let content = fs::read_to_string(&history_file)?;
        let index: HistoryIndex = serde_json::from_str(&content)?;
        if index.snapshots.iter().any(|m| m.id == id_or_tag) {
            return Ok(id_or_tag.to_string());
        }
        if let Some(meta) = index
            .snapshots
            .iter()
            .find(|m| m.tag.as_deref() == Some(id_or_tag))
        {
            return Ok(meta.id.clone());
        }
        anyhow::bail!("snapshot or tag '{}' not found in history", id_or_tag)
    }

    pub fn register_snapshot(&self, metadata: SnapshotMetadata) -> Result<()> {
        let history_file = self.odin_dir.join(".history");

        let mut index = if history_file.exists() {
            let content = fs::read_to_string(&history_file)?;
            serde_json::from_str(&content)?
        } else {
            HistoryIndex::new()
        };

        index.add_snapshot(metadata);

        let content = serde_json::to_string_pretty(&index)?;

        fs::write(&history_file, content)?;

        Ok(())
    }

    pub fn cleanup_old_snapshots(&self, keep_count: usize) -> Result<u32> {
        let history_file = self.odin_dir.join(".history");

        if !history_file.exists() {
            return Ok(0);
        }

        let content = fs::read_to_string(&history_file)?;

        let mut index: HistoryIndex = serde_json::from_str(&content)?;

        let original_count = index.snapshots.len();

        if original_count > keep_count {
            // Remove oldest snapshots
            let to_remove = original_count - keep_count;
            index.snapshots.truncate(keep_count);

            let content = serde_json::to_string_pretty(&index)?;

            fs::write(&history_file, content)?;

            Ok(to_remove as u32)
        } else {
            Ok(0)
        }
    }

    fn compute_diff(&self, from_id: &str, to_id: &str) -> Result<Vec<EnvironmentChange>> {
        let from_file = self.odin_dir.join(format!("snapshot-{}.json", from_id));
        let to_file = self.odin_dir.join(format!("snapshot-{}.json", to_id));

        if !from_file.exists() || !to_file.exists() {
            return Err(anyhow::anyhow!(
                "Snapshot files not found: {} or {}",
                from_id,
                to_id
            ));
        }

        let from_content = fs::read_to_string(&from_file)?;
        let to_content = fs::read_to_string(&to_file)?;

        let from_snapshot: serde_json::Value = serde_json::from_str(&from_content)?;
        let to_snapshot: serde_json::Value = serde_json::from_str(&to_content)?;

        let mut changes = Vec::new();

        // Compare packages
        changes.extend(self.compare_packages(&from_snapshot, &to_snapshot)?);

        // Compare environment variables
        changes.extend(self.compare_env_vars(&from_snapshot, &to_snapshot)?);

        // Compare VS Code extensions
        changes.extend(self.compare_vscode(&from_snapshot, &to_snapshot)?);

        // Compare Git config
        changes.extend(self.compare_git(&from_snapshot, &to_snapshot)?);

        Ok(changes)
    }

    fn compare_packages(
        &self,
        from: &serde_json::Value,
        to: &serde_json::Value,
    ) -> Result<Vec<EnvironmentChange>> {
        let mut changes = Vec::new();

        let empty_vec = vec![];
        let from_packages = from
            .get("packages")
            .and_then(|p| p.as_array())
            .unwrap_or(&empty_vec);
        let to_packages = to
            .get("packages")
            .and_then(|p| p.as_array())
            .unwrap_or(&empty_vec);

        // Find removed and updated packages
        for from_pkg in from_packages {
            let from_name = from_pkg.get("name").and_then(|n| n.as_str());
            let from_version = from_pkg.get("version").and_then(|v| v.as_str());

            if let Some(name) = from_name {
                let found = to_packages
                    .iter()
                    .find(|p| p.get("name").and_then(|n| n.as_str()) == from_name);

                if found.is_none() {
                    changes.push(EnvironmentChange {
                        change_type: ChangeType::Removed,
                        category: "package".to_string(),
                        item: name.to_string(),
                        old_value: from_version.map(String::from),
                        new_value: None,
                        details: None,
                    });
                } else if let Some(to_pkg) = found {
                    let to_version = to_pkg.get("version").and_then(|v| v.as_str());
                    if from_version != to_version {
                        changes.push(EnvironmentChange {
                            change_type: ChangeType::Updated,
                            category: "package".to_string(),
                            item: name.to_string(),
                            old_value: from_version.map(String::from),
                            new_value: to_version.map(String::from),
                            details: None,
                        });
                    }
                }
            }
        }

        // Find added packages
        for to_pkg in to_packages {
            let to_name = to_pkg.get("name").and_then(|n| n.as_str());
            let to_version = to_pkg.get("version").and_then(|v| v.as_str());

            if let Some(name) = to_name {
                let found = from_packages
                    .iter()
                    .find(|p| p.get("name").and_then(|n| n.as_str()) == to_name);

                if found.is_none() {
                    changes.push(EnvironmentChange {
                        change_type: ChangeType::Added,
                        category: "package".to_string(),
                        item: name.to_string(),
                        old_value: None,
                        new_value: to_version.map(String::from),
                        details: None,
                    });
                }
            }
        }

        Ok(changes)
    }

    fn compare_env_vars(
        &self,
        from: &serde_json::Value,
        to: &serde_json::Value,
    ) -> Result<Vec<EnvironmentChange>> {
        let mut changes = Vec::new();

        let empty_map = Default::default();
        let from_env = from
            .get("environment")
            .and_then(|e| e.as_object())
            .unwrap_or(&empty_map);
        let to_env = to
            .get("environment")
            .and_then(|e| e.as_object())
            .unwrap_or(&empty_map);

        // Check for removed or changed env vars
        for (key, from_val) in from_env {
            let from_str = from_val.as_str();
            let to_val = to_env.get(key);

            match to_val {
                None => {
                    changes.push(EnvironmentChange {
                        change_type: ChangeType::Removed,
                        category: "env_var".to_string(),
                        item: key.clone(),
                        old_value: from_str.map(String::from),
                        new_value: None,
                        details: None,
                    });
                }
                Some(to_str) if to_str.as_str() != from_str => {
                    changes.push(EnvironmentChange {
                        change_type: ChangeType::Modified,
                        category: "env_var".to_string(),
                        item: key.clone(),
                        old_value: from_str.map(String::from),
                        new_value: to_str.as_str().map(String::from),
                        details: None,
                    });
                }
                _ => {}
            }
        }

        // Check for added env vars
        for (key, to_val) in to_env {
            if !from_env.contains_key(key) {
                changes.push(EnvironmentChange {
                    change_type: ChangeType::Added,
                    category: "env_var".to_string(),
                    item: key.clone(),
                    old_value: None,
                    new_value: to_val.as_str().map(String::from),
                    details: None,
                });
            }
        }

        Ok(changes)
    }

    fn compare_vscode(
        &self,
        from: &serde_json::Value,
        to: &serde_json::Value,
    ) -> Result<Vec<EnvironmentChange>> {
        let mut changes = Vec::new();

        let empty_vec = vec![];
        let from_exts = from
            .get("vscode_extensions")
            .and_then(|e| e.as_array())
            .unwrap_or(&empty_vec);
        let to_exts = to
            .get("vscode_extensions")
            .and_then(|e| e.as_array())
            .unwrap_or(&empty_vec);

        // Find removed extensions
        for from_ext in from_exts {
            let from_id = from_ext.get("id").and_then(|i| i.as_str());

            if let Some(id) = from_id {
                if !to_exts
                    .iter()
                    .any(|e| e.get("id").and_then(|i| i.as_str()) == from_id)
                {
                    changes.push(EnvironmentChange {
                        change_type: ChangeType::Removed,
                        category: "vscode_extension".to_string(),
                        item: id.to_string(),
                        old_value: from_ext
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        new_value: None,
                        details: None,
                    });
                }
            }
        }

        // Find added extensions
        for to_ext in to_exts {
            let to_id = to_ext.get("id").and_then(|i| i.as_str());

            if let Some(id) = to_id {
                if !from_exts
                    .iter()
                    .any(|e| e.get("id").and_then(|i| i.as_str()) == to_id)
                {
                    changes.push(EnvironmentChange {
                        change_type: ChangeType::Added,
                        category: "vscode_extension".to_string(),
                        item: id.to_string(),
                        old_value: None,
                        new_value: to_ext
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        details: None,
                    });
                }
            }
        }

        Ok(changes)
    }

    fn compare_git(
        &self,
        from: &serde_json::Value,
        to: &serde_json::Value,
    ) -> Result<Vec<EnvironmentChange>> {
        let mut changes = Vec::new();

        let empty_map = Default::default();
        let from_config = from
            .get("git_config")
            .and_then(|g| g.as_object())
            .unwrap_or(&empty_map);
        let to_config = to
            .get("git_config")
            .and_then(|g| g.as_object())
            .unwrap_or(&empty_map);

        // Check for git config changes
        for (key, from_val) in from_config {
            let from_str = from_val.as_str();
            let to_val = to_config.get(key);

            if let Some(to_str) = to_val {
                if to_str.as_str() != from_str {
                    changes.push(EnvironmentChange {
                        change_type: ChangeType::Modified,
                        category: "git_config".to_string(),
                        item: key.clone(),
                        old_value: from_str.map(String::from),
                        new_value: to_str.as_str().map(String::from),
                        details: None,
                    });
                }
            }
        }

        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_service_creation() {
        let service = HistoryService::new("/tmp");
        assert!(service.odin_dir.ends_with("/tmp"));
    }
}
