use std::env;

use anyhow::Result;
use chrono::Utc;
use sysinfo::System;

use crate::integrations::{git_cli, package_managers, powershell, process, vscode, windows};
use crate::models::history::SnapshotMetadata;
use crate::models::machine::{DeveloperTool, MachineSnapshot};
use crate::services::{export_service, history_service::HistoryService, storage::SnapshotStore};
use crate::utils::paths;

pub struct SnapshotService {
    store: SnapshotStore,
    keep_last: Option<usize>,
}

impl SnapshotService {
    pub fn new(store: SnapshotStore) -> Self {
        Self {
            store,
            keep_last: None,
        }
    }

    pub fn with_keep_last(mut self, keep_last: Option<usize>) -> Self {
        self.keep_last = keep_last;
        self
    }

    pub async fn capture(
        &self,
        include_machine_env: bool,
        tag: Option<String>,
    ) -> Result<MachineSnapshot> {
        let snapshot_id = uuid::Uuid::new_v4();
        let machine = collect_machine(snapshot_id).await?;
        let environment = windows::environment(include_machine_env).await?;
        let packages = package_managers::list_packages().await?;
        let vscode = vscode::list_extensions().await?;
        let git = git_cli::global_config().await?;

        self.store
            .write_snapshot(&machine, &environment, &packages, &vscode, &git)
            .await?;
        export_service::ExportService::new(self.store.clone())
            .export_scripts(true)
            .await?;

        let history_root = self
            .store
            .root()
            .join("history")
            .join(snapshot_id.to_string());
        let history_store = SnapshotStore::new(history_root);
        history_store
            .write_snapshot(&machine, &environment, &packages, &vscode, &git)
            .await?;

        let history_service = HistoryService::new(self.store.root());
        history_service.register_snapshot(SnapshotMetadata {
            id: snapshot_id.to_string(),
            timestamp: machine.captured_at.to_rfc3339(),
            hostname: machine.hostname.clone(),
            os_version: machine.os_version.clone(),
            total_packages: packages.packages.len(),
            tag,
        })?;

        if let Some(keep) = self.keep_last {
            history_service.cleanup_old_snapshots(keep)?;
        }

        Ok(machine)
    }
}

async fn collect_machine(snapshot_id: uuid::Uuid) -> Result<MachineSnapshot> {
    let mut system = System::new_all();
    system.refresh_all();
    let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
    let username = env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_string());
    let cpu_brand = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let package_managers = package_managers::detect_managers().await;
    let developer_tools = detect_developer_tools().await;
    let shell = env::var("SHELL")
        .or_else(|_| env::var("ComSpec"))
        .unwrap_or_else(|_| powershell::executable().unwrap_or_else(|| "unknown".to_string()));

    Ok(MachineSnapshot {
        snapshot_id,
        captured_at: Utc::now(),
        hostname,
        username,
        os_name: System::name().unwrap_or_else(|| "Windows".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
        cpu_brand,
        cpu_count: system.cpus().len(),
        total_memory_bytes: system.total_memory(),
        shell,
        package_managers,
        developer_tools,
        powershell_profile_path: powershell::profile_path_lossy()
            .await
            .map(|p| p.to_string_lossy().to_string()),
        terminal_settings_path: windows::terminal_settings().await?.map(|p| p.path),
    })
}

async fn detect_developer_tools() -> Vec<DeveloperTool> {
    let tools = [
        (
            "Git",
            "git",
            &["--version"][..],
            Some("winget install --id Git.Git --exact"),
        ),
        (
            "Rust",
            "rustc",
            &["--version"][..],
            Some("winget install --id Rustlang.Rustup --exact"),
        ),
        (
            "Cargo",
            "cargo",
            &["--version"][..],
            Some("winget install --id Rustlang.Rustup --exact"),
        ),
        (
            "Node.js",
            "node",
            &["--version"][..],
            Some("winget install --id OpenJS.NodeJS.LTS --exact"),
        ),
        (
            "npm",
            "npm",
            &["--version"][..],
            Some("winget install --id OpenJS.NodeJS.LTS --exact"),
        ),
        (
            "Python",
            "python",
            &["--version"][..],
            Some("winget install --id Python.Python.3.12 --exact"),
        ),
        (
            "Go",
            "go",
            &["version"][..],
            Some("winget install --id GoLang.Go --exact"),
        ),
        (
            "Docker",
            "docker",
            &["--version"][..],
            Some("winget install --id Docker.DockerDesktop --exact"),
        ),
        (
            "VS Code",
            "code",
            &["--version"][..],
            Some("winget install --id Microsoft.VisualStudioCode --exact"),
        ),
        (
            "PowerShell",
            "pwsh",
            &["--version"][..],
            Some("winget install --id Microsoft.PowerShell --exact"),
        ),
    ];

    let mut detected = Vec::new();
    for (name, executable, version_args, install_command) in tools {
        let path = which::which(executable)
            .ok()
            .map(|p| p.to_string_lossy().to_string());
        if path.is_none() {
            continue;
        }
        let version = process::capture(executable, version_args)
            .await
            .ok()
            .map(|out| out.stdout)
            .filter(|s| !s.is_empty());
        detected.push(DeveloperTool {
            name: name.to_string(),
            executable: executable.to_string(),
            path,
            version,
            install_source: Some("PATH".to_string()),
            install_command: install_command.map(ToOwned::to_owned),
        });
    }

    if let Ok(profile) = paths::user_profile() {
        let manual_dirs = [
            profile.join("scoop"),
            profile.join(".cargo"),
            profile.join(".dotnet"),
        ];
        for dir in manual_dirs {
            if dir.exists() {
                detected.push(DeveloperTool {
                    name: dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("manual-tool")
                        .to_string(),
                    executable: String::new(),
                    path: Some(dir.to_string_lossy().to_string()),
                    version: None,
                    install_source: Some("manual-directory".to_string()),
                    install_command: None,
                });
            }
        }
    }

    detected
}
