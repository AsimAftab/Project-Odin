use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::integrations::process;
use crate::models::vscode::{VsCodeExtension, VsCodeExtensionsSnapshot};

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
    let Some(code) = executable() else {
        return Ok(VsCodeExtensionsSnapshot {
            extensions: Vec::new(),
        });
    };

    let output = match process::capture(&code, &["--list-extensions", "--show-versions"]).await {
        Ok(output) => output,
        Err(error) => {
            eprintln!("warning: VS Code extension probe failed: {error:#}");
            return Ok(VsCodeExtensionsSnapshot {
                extensions: Vec::new(),
            });
        }
    };
    if output.code != 0 {
        return Ok(VsCodeExtensionsSnapshot {
            extensions: Vec::new(),
        });
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

    Ok(VsCodeExtensionsSnapshot { extensions })
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
