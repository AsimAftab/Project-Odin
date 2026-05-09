use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineSnapshot {
    pub snapshot_id: uuid::Uuid,
    pub captured_at: DateTime<Utc>,
    pub hostname: String,
    pub username: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub cpu_brand: String,
    pub cpu_count: usize,
    pub total_memory_bytes: u64,
    pub shell: String,
    pub package_managers: Vec<PackageManagerInfo>,
    pub developer_tools: Vec<DeveloperTool>,
    pub powershell_profile_path: Option<String>,
    pub terminal_settings_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManagerInfo {
    pub name: String,
    pub installed: bool,
    pub executable: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperTool {
    pub name: String,
    pub executable: String,
    pub path: Option<String>,
    pub version: Option<String>,
    pub install_source: Option<String>,
    pub install_command: Option<String>,
}
