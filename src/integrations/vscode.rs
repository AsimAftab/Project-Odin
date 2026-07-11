use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::integrations::process;
use crate::models::environment::ProfileSnapshot;
use crate::models::vscode::{VsCodeExtension, VsCodeExtensionsSnapshot};
use crate::utils::checksum;

pub fn executable() -> Option<String> {
    if process::command_exists("code") {
        return Some("code".to_string());
    }

    for candidate in code_candidates() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

pub async fn list_extensions() -> Result<VsCodeExtensionsSnapshot> {
    let user_config = capture_user_config().await;

    let Some(code) = executable() else {
        return Ok(user_config);
    };

    let output = match process::capture(&code, &["--list-extensions", "--show-versions"]).await {
        Ok(output) => output,
        Err(error) => {
            eprintln!("warning: VS Code extension probe failed: {error:#}");
            return Ok(user_config);
        }
    };
    if output.code != 0 {
        return Ok(user_config);
    }

    let extensions = output
        .stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.split('@');
            let identifier = parts.next()?.to_string();
            let version = parts.next().map(ToOwned::to_owned);
            Some(VsCodeExtension {
                identifier,
                version,
            })
        })
        .collect();

    Ok(VsCodeExtensionsSnapshot {
        extensions,
        ..user_config
    })
}

/// VS Code's user configuration directory on this machine
/// (`%APPDATA%\Code\User`), or None when VS Code has never run here.
pub fn user_config_dir() -> Option<PathBuf> {
    let app_data = std::env::var("APPDATA").ok()?;
    let dir = Path::new(&app_data).join(r"Code\User");
    dir.exists().then_some(dir)
}

async fn read_profile_file(path: &Path) -> Option<ProfileSnapshot> {
    if !path.exists() {
        return None;
    }
    let content = tokio::fs::read_to_string(path).await.ok()?;
    Some(ProfileSnapshot {
        path: path.to_string_lossy().to_string(),
        sha256: checksum::sha256_bytes(content.as_bytes()),
        content,
    })
}

/// Captures settings.json, keybindings.json, and snippets/*.json from the
/// user config dir. Extensions are filled in by the caller.
async fn capture_user_config() -> VsCodeExtensionsSnapshot {
    let mut snapshot = VsCodeExtensionsSnapshot::default();
    let Some(user_dir) = user_config_dir() else {
        return snapshot;
    };
    snapshot.settings = read_profile_file(&user_dir.join("settings.json")).await;
    snapshot.keybindings = read_profile_file(&user_dir.join("keybindings.json")).await;

    if let Ok(mut entries) = tokio::fs::read_dir(user_dir.join("snippets")).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let is_snippet = path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("json") || ext.eq_ignore_ascii_case("code-snippets")
            });
            if is_snippet {
                if let Some(profile) = read_profile_file(&path).await {
                    snapshot.snippets.push(profile);
                }
            }
        }
        snapshot.snippets.sort_by(|a, b| a.path.cmp(&b.path));
    }
    snapshot
}

fn code_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidates
            .push(Path::new(&local_app_data).join(r"Programs\Microsoft VS Code\bin\code.cmd"));
        candidates.push(
            Path::new(&local_app_data)
                .join(r"Programs\Microsoft VS Code Insiders\bin\code-insiders.cmd"),
        );
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(Path::new(&program_files).join(r"Microsoft VS Code\bin\code.cmd"));
    }
    candidates
}
