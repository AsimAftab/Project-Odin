use serde::{Deserialize, Serialize};

use crate::models::environment::ProfileSnapshot;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VsCodeExtensionsSnapshot {
    pub extensions: Vec<VsCodeExtension>,
    // User-level editor config. `default` keeps snapshots captured before
    // these fields existed loadable.
    #[serde(default)]
    pub settings: Option<ProfileSnapshot>,
    #[serde(default)]
    pub keybindings: Option<ProfileSnapshot>,
    /// One entry per `snippets/*.json` file; `path` records the file name's
    /// original absolute location.
    #[serde(default)]
    pub snippets: Vec<ProfileSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsCodeExtension {
    pub identifier: String,
    pub version: Option<String>,
}
