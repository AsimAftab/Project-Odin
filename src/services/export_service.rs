use anyhow::Result;

use crate::models::package::PackageManager;
use crate::services::storage::SnapshotStore;
use crate::utils::fs;

#[derive(Clone)]
pub struct ExportService {
    store: SnapshotStore,
}

impl ExportService {
    pub fn new(store: SnapshotStore) -> Self {
        Self { store }
    }

    pub async fn export_scripts(&self, overwrite: bool) -> Result<()> {
        let packages = self.store.read_packages().await?;
        let environment = self.store.read_environment().await?;
        let vscode = self.store.read_vscode().await?;
        let git = self.store.read_git().await?;

        let restore = render_restore_script(&packages, &environment, &vscode, &git);
        let install = render_install_script(&packages);
        let bootstrap = render_bootstrap_script();

        for (name, content) in [
            ("restore.ps1", restore),
            ("install.ps1", install),
            ("bootstrap.ps1", bootstrap),
        ] {
            let path = self.store.path(name);
            if overwrite || !path.exists() {
                fs::write_text(&path, &content).await?;
            }
        }
        Ok(())
    }
}

fn ps_escape(value: &str) -> String {
    value.replace('`', "``").replace('\'', "''")
}

fn render_install_script(packages: &crate::models::package::PackageSnapshot) -> String {
    let mut script = String::from("$ErrorActionPreference = 'Stop'\n\n");
    script.push_str("function Invoke-OdinCommand([string]$Command) {\n  Write-Host \"> $Command\" -ForegroundColor Cyan\n  Invoke-Expression $Command\n}\n\n");
    for package in &packages.packages {
        if let Some(command) = &package.install_command {
            script.push_str(&format!("Invoke-OdinCommand '{}'\n", ps_escape(command)));
        }
    }
    script
}

fn render_restore_script(
    packages: &crate::models::package::PackageSnapshot,
    environment: &crate::models::environment::EnvironmentSnapshot,
    vscode: &crate::models::vscode::VsCodeExtensionsSnapshot,
    git: &crate::models::git::GitConfigSnapshot,
) -> String {
    let mut script = render_install_script(packages);
    script.push_str("\n# Environment variables\n");
    for variable in &environment.user_variables {
        if variable.name.eq_ignore_ascii_case("PATH") {
            continue;
        }
        script.push_str(&format!(
            "[Environment]::SetEnvironmentVariable('{}', '{}', 'User')\n",
            ps_escape(&variable.name),
            ps_escape(&variable.value)
        ));
    }
    let path_value = environment
        .path_entries
        .iter()
        .map(|entry| entry.value.as_str())
        .collect::<Vec<_>>()
        .join(";");
    if !path_value.is_empty() {
        script.push_str(&format!(
            "[Environment]::SetEnvironmentVariable('Path', '{}', 'User')\n",
            ps_escape(&path_value)
        ));
    }

    if let Some(profile) = &environment.powershell_profile {
        script.push_str("\n# PowerShell profile\n");
        script.push_str(&format!(
            "New-Item -ItemType Directory -Force -Path (Split-Path '{}') | Out-Null\n",
            ps_escape(&profile.path)
        ));
        script.push_str(&format!(
            "@'\n{}\n'@ | Set-Content -Encoding UTF8 -Path '{}'\n",
            profile.content,
            ps_escape(&profile.path)
        ));
    }

    script.push_str("\n# VS Code extensions\n");
    for extension in &vscode.extensions {
        script.push_str(&format!(
            "code --install-extension '{}'\n",
            ps_escape(&extension.identifier)
        ));
    }

    script.push_str("\n# Git global config\n");
    for entry in &git.entries {
        script.push_str(&format!(
            "git config --global '{}' '{}'\n",
            ps_escape(&entry.key),
            ps_escape(&entry.value)
        ));
    }

    let has_winget = packages
        .packages
        .iter()
        .any(|p| p.source == PackageManager::Winget);
    if has_winget {
        script.push_str("\nWrite-Host 'winget packages were restored with source agreements accepted where required.'\n");
    }
    script
}

fn render_bootstrap_script() -> String {
    String::from(
        "$ErrorActionPreference = 'Stop'\n\
         if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {\n\
         \tWrite-Warning 'winget was not found. Install App Installer from Microsoft Store first.'\n\
         }\n\
         powershell -ExecutionPolicy Bypass -File .\\restore.ps1\n",
    )
}
