use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::models::diff::{DiffChange, DiffReport};
use crate::services::storage::SnapshotStore;

pub struct DiffService {
    store: SnapshotStore,
}

impl DiffService {
    pub fn new(store: SnapshotStore) -> Self {
        Self { store }
    }

    pub async fn diff(&self) -> Result<DiffReport> {
        let _machine = self.store.read_machine().await?;
        let snapshot_packages = self.store.read_packages().await?;
        let current_packages = crate::integrations::package_managers::list_packages().await?;
        let snapshot_env = self.store.read_environment().await?;
        let current_env = crate::integrations::windows::environment(false).await?;
        let snapshot_vscode = self.store.read_vscode().await?;
        let current_vscode = crate::integrations::vscode::list_extensions().await?;

        let mut changes = Vec::new();

        let before_packages = snapshot_packages
            .packages
            .iter()
            .map(|p| p.id.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let after_packages = current_packages
            .packages
            .iter()
            .map(|p| p.id.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        for item in before_packages.difference(&after_packages) {
            changes.push(DiffChange {
                category: "package".to_string(),
                item: item.clone(),
                before: Some("installed".to_string()),
                after: None,
            });
        }
        for item in after_packages.difference(&before_packages) {
            changes.push(DiffChange {
                category: "package".to_string(),
                item: item.clone(),
                before: None,
                after: Some("installed".to_string()),
            });
        }

        let before_env = snapshot_env
            .user_variables
            .iter()
            .map(|v| (v.name.to_ascii_lowercase(), v.value.clone()))
            .collect::<HashMap<_, _>>();
        let after_env = current_env
            .user_variables
            .iter()
            .map(|v| (v.name.to_ascii_lowercase(), v.value.clone()))
            .collect::<HashMap<_, _>>();
        for (name, before) in &before_env {
            if let Some(after) = after_env.get(name) {
                if before != after {
                    changes.push(DiffChange {
                        category: "env".to_string(),
                        item: name.clone(),
                        before: Some(before.clone()),
                        after: Some(after.clone()),
                    });
                }
            }
        }

        let before_ext = snapshot_vscode
            .extensions
            .iter()
            .map(|e| e.identifier.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let after_ext = current_vscode
            .extensions
            .iter()
            .map(|e| e.identifier.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        for item in before_ext.difference(&after_ext) {
            changes.push(DiffChange {
                category: "vscode".to_string(),
                item: item.clone(),
                before: Some("installed".to_string()),
                after: None,
            });
        }
        for item in after_ext.difference(&before_ext) {
            changes.push(DiffChange {
                category: "vscode".to_string(),
                item: item.clone(),
                before: None,
                after: Some("installed".to_string()),
            });
        }

        Ok(DiffReport { changes })
    }
}
