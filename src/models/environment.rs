use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    pub user_variables: Vec<EnvironmentVariable>,
    pub machine_variables: Vec<EnvironmentVariable>,
    pub path_entries: Vec<PathEntry>,
    pub powershell_profile: Option<ProfileSnapshot>,
    pub terminal_settings: Option<ProfileSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: String,
    pub scope: EnvironmentScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentScope {
    Process,
    User,
    Machine,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathEntry {
    pub value: String,
    pub exists: bool,
    pub source: EnvironmentScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    pub path: String,
    pub content: String,
    pub sha256: String,
}
