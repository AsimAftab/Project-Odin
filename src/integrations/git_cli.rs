use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::integrations::process;
use crate::models::git::{GitConfigEntry, GitConfigSnapshot};

pub fn executable() -> Option<String> {
    if process::command_exists("git") {
        return Some("git".to_string());
    }

    for candidate in git_candidates() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

pub async fn global_config() -> Result<GitConfigSnapshot> {
    let Some(git) = executable() else {
        return Ok(GitConfigSnapshot {
            entries: Vec::new(),
        });
    };

    let output =
        match process::capture(&git, &["config", "--global", "--list", "--show-origin"]).await {
            Ok(output) => output,
            Err(error) => {
                eprintln!("warning: Git config probe failed: {error:#}");
                return Ok(GitConfigSnapshot {
                    entries: Vec::new(),
                });
            }
        };
    if output.code != 0 {
        return Ok(GitConfigSnapshot {
            entries: Vec::new(),
        });
    }

    let entries = output
        .stdout
        .lines()
        .filter_map(|line| {
            let (origin, rest) = line.split_once('\t').map_or((None, line), |(left, right)| {
                (Some(left.to_string()), right)
            });
            let (key, value) = rest.split_once('=')?;
            Some(GitConfigEntry {
                key: key.to_string(),
                value: value.to_string(),
                origin,
            })
        })
        .collect();

    Ok(GitConfigSnapshot { entries })
}

pub async fn init_repo(path: &std::path::Path) -> Result<()> {
    if !path.join(".git").exists() {
        let git = executable().unwrap_or_else(|| "git".to_string());
        let root = path.to_string_lossy().to_string();
        process::checked(&git, &["init", &root]).await?;
    }
    Ok(())
}

fn git_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(Path::new(&program_files).join(r"Git\cmd\git.exe"));
        candidates.push(Path::new(&program_files).join(r"Git\bin\git.exe"));
    }
    if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
        candidates.push(Path::new(&program_files_x86).join(r"Git\cmd\git.exe"));
        candidates.push(Path::new(&program_files_x86).join(r"Git\bin\git.exe"));
    }
    candidates
}
