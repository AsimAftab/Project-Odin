use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata about a snapshot including timestamp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotMetadata {
    pub id: String,
    pub timestamp: String, // ISO 8601 format
    pub hostname: String,
    pub os_version: String,
    pub total_packages: usize,
    /// Optional human-readable tag, e.g. "prod", "before-migration".
    #[serde(default)]
    pub tag: Option<String>,
}

/// Type of environment change
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Added,
    Removed,
    Updated,
    Modified,
}

/// A single change in the environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentChange {
    pub change_type: ChangeType,
    pub category: String, // "package", "env_var", "vscode_extension", "git_config"
    pub item: String,     // package name, env var name, extension id, etc.
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub details: Option<String>,
}

/// Differences between two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub from_snapshot: String,
    pub to_snapshot: String,
    pub changes: Vec<EnvironmentChange>,
    pub summary: DiffSummary,
}

/// Summary of changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub packages_added: usize,
    pub packages_removed: usize,
    pub packages_updated: usize,
    pub env_vars_changed: usize,
    pub extensions_added: usize,
    pub extensions_removed: usize,
    pub other_changes: usize,
}

impl DiffSummary {
    pub fn from_changes(changes: &[EnvironmentChange]) -> Self {
        let mut summary = DiffSummary {
            packages_added: 0,
            packages_removed: 0,
            packages_updated: 0,
            env_vars_changed: 0,
            extensions_added: 0,
            extensions_removed: 0,
            other_changes: 0,
        };

        for change in changes {
            match (change.category.as_str(), change.change_type) {
                ("package", ChangeType::Added) => summary.packages_added += 1,
                ("package", ChangeType::Removed) => summary.packages_removed += 1,
                ("package", ChangeType::Updated) => summary.packages_updated += 1,
                ("env_var", ChangeType::Modified) => summary.env_vars_changed += 1,
                ("vscode_extension", ChangeType::Added) => summary.extensions_added += 1,
                ("vscode_extension", ChangeType::Removed) => summary.extensions_removed += 1,
                _ => summary.other_changes += 1,
            }
        }

        summary
    }
}

/// A single history entry with metadata and changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub metadata: SnapshotMetadata,
    pub changes: Vec<EnvironmentChange>,
    pub summary: DiffSummary,
}

/// History index containing all snapshots
#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryIndex {
    pub snapshots: Vec<SnapshotMetadata>,
    pub diffs: HashMap<String, SnapshotDiff>, // Key: "snapshot_a-to-snapshot_b"
}

#[allow(dead_code)]
impl HistoryIndex {
    pub fn new() -> Self {
        HistoryIndex {
            snapshots: Vec::new(),
            diffs: HashMap::new(),
        }
    }

    /// Add a new snapshot to history
    pub fn add_snapshot(&mut self, metadata: SnapshotMetadata) {
        self.snapshots.push(metadata);
        // Keep snapshots sorted by timestamp (newest first)
        self.snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    /// Cache a diff between two snapshots
    pub fn cache_diff(&mut self, diff: SnapshotDiff) {
        let key = format!("{}-to-{}", diff.from_snapshot, diff.to_snapshot);
        self.diffs.insert(key, diff);
    }

    /// Get diff between two snapshots
    pub fn get_diff(&self, from: &str, to: &str) -> Option<&SnapshotDiff> {
        let key = format!("{}-to-{}", from, to);
        self.diffs.get(&key)
    }

    /// Get snapshot by ID
    pub fn get_snapshot(&self, id: &str) -> Option<&SnapshotMetadata> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// Get all snapshots sorted by date (newest first)
    pub fn get_all_snapshots(&self) -> &[SnapshotMetadata] {
        &self.snapshots
    }

    /// Get the most recent snapshot
    pub fn get_latest(&self) -> Option<&SnapshotMetadata> {
        self.snapshots.first()
    }

    /// Get the oldest snapshot
    pub fn get_oldest(&self) -> Option<&SnapshotMetadata> {
        self.snapshots.last()
    }
}

impl Default for HistoryIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_summary_counts() {
        let changes = vec![
            EnvironmentChange {
                change_type: ChangeType::Added,
                category: "package".to_string(),
                item: "python".to_string(),
                old_value: None,
                new_value: Some("3.12".to_string()),
                details: None,
            },
            EnvironmentChange {
                change_type: ChangeType::Removed,
                category: "package".to_string(),
                item: "old-tool".to_string(),
                old_value: Some("1.0".to_string()),
                new_value: None,
                details: None,
            },
        ];

        let summary = DiffSummary::from_changes(&changes);
        assert_eq!(summary.packages_added, 1);
        assert_eq!(summary.packages_removed, 1);
    }

    #[test]
    fn test_history_index_ordering() {
        let mut index = HistoryIndex::new();

        let meta1 = SnapshotMetadata {
            id: "snap-1".to_string(),
            timestamp: "2024-05-10T10:00:00Z".to_string(),
            hostname: "PC1".to_string(),
            os_version: "Windows 11".to_string(),
            total_packages: 50,
            tag: None,
        };

        let meta2 = SnapshotMetadata {
            id: "snap-2".to_string(),
            timestamp: "2024-05-11T10:00:00Z".to_string(),
            hostname: "PC1".to_string(),
            os_version: "Windows 11".to_string(),
            total_packages: 52,
            tag: None,
        };

        index.add_snapshot(meta1);
        index.add_snapshot(meta2);

        // Should be sorted by timestamp (newest first)
        assert_eq!(index.get_latest().unwrap().id, "snap-2");
        assert_eq!(index.get_oldest().unwrap().id, "snap-1");
    }
}
