use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;

use crate::integrations::{
    git_cli, install, package_managers, powershell, process, vscode, windows,
};
use crate::models::doctor::{DoctorFinding, DoctorReport, Severity};

pub struct DoctorService;

impl DoctorService {
    pub async fn run() -> Result<DoctorReport> {
        let env = windows::environment(false).await?;
        let install_status = install::collect_status().await?;
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

        if !install_status.current_executable.exists() {
            findings.push(DoctorFinding {
                severity: Severity::Error,
                code: "odin-executable-missing".to_string(),
                message: format!(
                    "Current Odin executable path does not exist: {}",
                    install_status.current_executable.display()
                ),
                suggestion: Some(
                    "Reinstall Odin using install.ps1 or your package manager.".to_string(),
                ),
            });
        } else {
            let executable = install_status
                .current_executable
                .to_string_lossy()
                .to_string();
            let output = process::capture(&executable, &["--version"]).await?;
            if output.code != 0 {
                findings.push(DoctorFinding {
                    severity: Severity::Error,
                    code: "odin-not-executable".to_string(),
                    message: format!(
                        "Odin executable failed to run from {}",
                        install_status.current_executable.display()
                    ),
                    suggestion: Some(
                        "Reinstall Odin to replace the executable with a healthy build."
                            .to_string(),
                    ),
                });
            }
        }

        if !process::command_exists("odin") {
            findings.push(DoctorFinding {
                severity: Severity::Warning,
                code: "odin-not-on-path".to_string(),
                message: "The `odin` command is not available from PATH.".to_string(),
                suggestion: Some(
                    "Add the Odin install directory to User/System PATH or reinstall Odin."
                        .to_string(),
                ),
            });
        }

        if !install_status.process_has_current_directory {
            findings.push(DoctorFinding {
                severity: Severity::Warning,
                code: "odin-dir-not-on-path".to_string(),
                message: format!(
                    "Current Odin executable directory is missing from PATH: {}",
                    install_status.current_directory.display()
                ),
                suggestion: Some(
                    "Add this directory to PATH or reinstall Odin so it configures PATH automatically."
                        .to_string(),
                ),
            });
        }

        if !install_status.user_path_has_user_install_dir
            && !install_status.machine_path_has_machine_install_dir
        {
            findings.push(DoctorFinding {
                severity: Severity::Warning,
                code: "odin-persistent-path-missing".to_string(),
                message: format!(
                    "Default Odin install paths are missing from persistent PATH: {} / {}",
                    install_status.user_install_dir.display(),
                    install_status.machine_install_dir.display()
                ),
                suggestion: Some(
                    "Run install.ps1 again to configure PATH for your selected install scope."
                        .to_string(),
                ),
            });
        }

        for duplicate in install::path_duplicates(&install::odin_path_entries(
            &install_status.process_path_entries,
        )) {
            findings.push(DoctorFinding {
                severity: Severity::Info,
                code: "duplicate-odin-path".to_string(),
                message: format!("Odin PATH entry appears multiple times: {duplicate}"),
                suggestion: Some("Keep one Odin PATH entry to avoid ambiguity.".to_string()),
            });
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
