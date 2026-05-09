use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsCodeExtensionsSnapshot {
    pub extensions: Vec<VsCodeExtension>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsCodeExtension {
    pub identifier: String,
    pub version: Option<String>,
}
