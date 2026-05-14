use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatmodeConfig {
    #[serde(default)]
    pub profiles: BTreeMap<String, Vec<BatmodeEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatmodeEntry {
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl BatmodeEntry {
    pub fn display_name(&self) -> String {
        std::path::Path::new(&self.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.path.clone())
    }
}
