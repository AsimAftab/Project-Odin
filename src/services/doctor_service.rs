use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;

use crate::integrations::{git_cli, package_managers, powershell, process, vscode, windows};
use crate::models::doctor::{DoctorFinding, DoctorReport, Severity};

pub struct DoctorService;

impl DoctorService {
    pub async fn run() -> Result<DoctorReport> {
        let env = windows::environment(false).await?;
        let mut findings = Vec::new();

        let mut seen = HashSet::new();
        for entry in &env.path_entries {
            let normalized = entry.value.to_ascii_lowercase();
            if !entry.exists {
                findings.push(DoctorFinding {
                    severity: Severity::Warning,
                    code: "broken-path".to_string(),
                    message: format!("PATH entry does not exist: {}", entry.value),
                    suggestion: Some(
                        "Remove the stale entry or reinstall the owning tool.".to_string(),
                    ),
                });
            }
            if !seen.insert(normalized) {
                findings.push(DoctorFinding {
                    severity: Severity::Info,
                    code: "duplicate-path".to_string(),
                    message: format!("PATH entry appears more than once: {}", entry.value),
                    suggestion: Some(
                        "Keep one canonical PATH entry to reduce command resolution ambiguity."
                            .to_string(),
                    ),
                });
            }
        }

        if git_cli::executable().is_none() {
            findings.push(DoctorFinding {
                severity: Severity::Warning,
                code: "missing-tool".to_string(),
                message: "Git was not found in PATH or standard install locations.".to_string(),
                suggestion: Some(
                    "Install Git with `winget install --id Git.Git --exact`.".to_string(),
                ),
            });
        }

        if vscode::executable().is_none() {
            findings.push(DoctorFinding {
                severity: Severity::Warning,
                code: "missing-tool".to_string(),
                message: "VS Code was not found in PATH or standard install locations.".to_string(),
                suggestion: Some(
                    "Install VS Code with `winget install --id Microsoft.VisualStudioCode --exact`."
                        .to_string(),
                ),
            });
        }

        if powershell::executable().is_none() {
            findings.push(DoctorFinding {
                severity: Severity::Warning,
                code: "missing-tool".to_string(),
                message: "PowerShell was not found in PATH or standard install locations."
                    .to_string(),
                suggestion: Some(
                    "Install PowerShell with `winget install --id Microsoft.PowerShell --exact`."
                        .to_string(),
                ),
            });
        }

        let packages = package_managers::list_packages().await?;
        let mut runtime_versions = HashSet::new();
        for package in packages.packages {
            let id = package.id.to_ascii_lowercase();
            if (id.contains("node") || id.contains("python") || id.contains("jdk"))
                && !runtime_versions.insert(id.clone())
            {
                findings.push(DoctorFinding {
                    severity: Severity::Info,
                    code: "duplicate-runtime".to_string(),
                    message: format!("Runtime package appears duplicated: {}", package.id),
                    suggestion: Some(
                        "Review package managers and keep one owner for each runtime.".to_string(),
                    ),
                });
            }
        }

        if !PathBuf::from(r"C:\Program Files\dotnet").exists() && !process::command_exists("dotnet")
        {
            findings.push(DoctorFinding {
                severity: Severity::Info,
                code: "missing-sdk".to_string(),
                message: ".NET SDK was not detected.".to_string(),
                suggestion: Some("Install it with `winget install --id Microsoft.DotNet.SDK.8 --exact` if needed.".to_string()),
            });
        }

        Ok(DoctorReport { findings })
    }
}
