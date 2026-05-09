use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfigSnapshot {
    pub entries: Vec<GitConfigEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfigEntry {
    pub key: String,
    pub value: String,
    pub origin: Option<String>,
}
