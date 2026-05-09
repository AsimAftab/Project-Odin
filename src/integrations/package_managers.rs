use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::integrations::process;
use crate::models::machine::PackageManagerInfo;
use crate::models::package::{InstalledPackage, PackageManager, PackageSnapshot};

pub async fn detect_managers() -> Vec<PackageManagerInfo> {
    let checks = [
        ("winget", executable("winget")),
        ("choco", choco_executable()),
        ("scoop", scoop_executable()),
    ];
    let mut managers = Vec::new();
    for (name, executable) in checks {
        let installed = executable.is_some();
        let version = if installed {
            process::capture(executable.as_deref().unwrap_or(name), &["--version"])
                .await
                .ok()
                .map(|out| out.stdout)
                .filter(|s| !s.is_empty())
        } else {
            None
        };
        managers.push(PackageManagerInfo {
            name: name.to_string(),
            installed,
            executable,
            version,
        });
    }
    managers
}

pub async fn list_packages() -> Result<PackageSnapshot> {
    let mut packages = Vec::new();
    for result in [list_winget().await, list_choco().await, list_scoop().await] {
        match result {
            Ok(mut manager_packages) => packages.append(&mut manager_packages),
            Err(error) => eprintln!("warning: package manager probe failed: {error:#}"),
        }
    }
    packages.sort_by(|left, right| left.id.cmp(&right.id));
    packages.dedup_by(|left, right| {
        left.id.eq_ignore_ascii_case(&right.id) && left.source == right.source
    });
    Ok(PackageSnapshot { packages })
}

async fn list_winget() -> Result<Vec<InstalledPackage>> {
    let Some(winget) = executable("winget") else {
        return Ok(Vec::new());
    };
    let temp_path = std::env::temp_dir().join(format!("odin-winget-{}.json", uuid::Uuid::new_v4()));
    let temp_file = temp_path.to_string_lossy().to_string();
    let output = process::capture(
        &winget,
        &[
            "export",
            "-o",
            &temp_file,
            "--include-versions",
            "--accept-source-agreements",
        ],
    )
    .await?;
    if output.code != 0 || !temp_path.exists() {
        return Ok(Vec::new());
    }
    let data = tokio::fs::read_to_string(&temp_path)
        .await
        .unwrap_or_default();
    let _ = tokio::fs::remove_file(&temp_path).await;
    let json: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
    let packages = json
        .get("Sources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|source| source.get("Packages").and_then(Value::as_array).into_iter().flatten())
        .filter_map(|pkg| {
            let id = pkg.get("PackageIdentifier")?.as_str()?.to_string();
            let version = pkg.get("Version").and_then(Value::as_str).map(ToOwned::to_owned);
            Some(InstalledPackage {
                name: id.clone(),
                install_command: Some(format!("winget install --id {id} --exact --accept-package-agreements --accept-source-agreements")),
                id,
                version,
                source: PackageManager::Winget,
            })
        })
        .collect();
    Ok(packages)
}

async fn list_choco() -> Result<Vec<InstalledPackage>> {
    let Some(choco) = choco_executable() else {
        return Ok(Vec::new());
    };
    let output = process::capture(&choco, &["list", "--local-only", "--limit-output"]).await?;
    if output.code != 0 {
        return Ok(Vec::new());
    }
    Ok(output
        .stdout
        .lines()
        .filter_map(|line| {
            let (id, version) = line.split_once('|')?;
            Some(InstalledPackage {
                id: id.to_string(),
                name: id.to_string(),
                version: Some(version.to_string()),
                source: PackageManager::Chocolatey,
                install_command: Some(format!("choco install {id} -y")),
            })
        })
        .collect())
}

async fn list_scoop() -> Result<Vec<InstalledPackage>> {
    let Some(scoop) = scoop_executable() else {
        return Ok(Vec::new());
    };
    let output = process::capture(&scoop, &["export"]).await?;
    if output.code != 0 {
        return Ok(Vec::new());
    }
    let json: Value = serde_json::from_str(&output.stdout).unwrap_or(Value::Null);
    let packages = json
        .get("apps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|app| {
            let name = app
                .get("Name")
                .or_else(|| app.get("name"))?
                .as_str()?
                .to_string();
            let version = app
                .get("Version")
                .or_else(|| app.get("version"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            Some(InstalledPackage {
                id: name.clone(),
                name: name.clone(),
                version,
                source: PackageManager::Scoop,
                install_command: Some(format!("scoop install {name}")),
            })
        })
        .collect();
    Ok(packages)
}

fn executable(name: &str) -> Option<String> {
    if process::command_exists(name) {
        Some(name.to_string())
    } else {
        None
    }
}

fn scoop_executable() -> Option<String> {
    if process::command_exists("scoop") {
        return Some("scoop".to_string());
    }

    for candidate in scoop_candidates() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn choco_executable() -> Option<String> {
    if process::command_exists("choco") {
        return Some("choco".to_string());
    }

    for candidate in choco_candidates() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn choco_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(chocolatey_install) = std::env::var("ChocolateyInstall") {
        candidates.push(Path::new(&chocolatey_install).join(r"bin\choco.exe"));
        candidates.push(Path::new(&chocolatey_install).join(r"bin\choco.bat"));
    }
    if let Ok(program_data) = std::env::var("ProgramData") {
        candidates.push(Path::new(&program_data).join(r"chocolatey\bin\choco.exe"));
        candidates.push(Path::new(&program_data).join(r"chocolatey\bin\choco.bat"));
    }
    candidates
}

fn scoop_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        candidates.push(Path::new(&user_profile).join(r"scoop\shims\scoop.cmd"));
        candidates.push(Path::new(&user_profile).join(r"scoop\shims\scoop.ps1"));
    }
    if let Ok(scoop) = std::env::var("SCOOP") {
        candidates.push(Path::new(&scoop).join(r"shims\scoop.cmd"));
        candidates.push(Path::new(&scoop).join(r"shims\scoop.ps1"));
    }
    candidates
}
