use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdinLock {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub snapshot_id: uuid::Uuid,
    pub files: Vec<LockedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedFile {
    pub path: String,
    pub sha256: String,
}
