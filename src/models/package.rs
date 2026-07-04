use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Winget,
    Chocolatey,
    Scoop,
    Npm,
    Pip,
    Cargo,
    Pipx,
    Pnpm,
    Yarn,
    #[serde(rename = "dotnet")]
    DotnetTool,
    Go,
    Uv,
    Manual,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSnapshot {
    pub packages: Vec<InstalledPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub source: PackageManager,
    pub install_command: Option<String>,
}
