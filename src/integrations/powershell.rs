use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::integrations::process;
use crate::models::environment::ProfileSnapshot;
use crate::utils::checksum;

pub fn executable() -> Option<String> {
    if process::command_exists("pwsh") {
        return Some("pwsh".to_string());
    }
    if process::command_exists("powershell") {
        return Some("powershell".to_string());
    }

    for candidate in powershell_candidates() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

pub async fn profile_path() -> Result<Option<PathBuf>> {
    let Some(executable) = executable() else {
        return Ok(None);
    };
    let output = process::capture(&executable, &["-NoProfile", "-Command", "$PROFILE"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(output.stdout)))
}

pub async fn read_profile() -> Result<Option<ProfileSnapshot>> {
    let path = match profile_path().await {
        Ok(Some(path)) => path,
        Ok(None) => return Ok(None),
        Err(error) => {
            eprintln!("warning: PowerShell profile probe failed: {error:#}");
            return Ok(None);
        }
    };
    if !path.exists() {
        return Ok(Some(ProfileSnapshot {
            path: path.to_string_lossy().to_string(),
            content: String::new(),
            sha256: checksum::sha256_bytes(b""),
        }));
    }
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    Ok(Some(ProfileSnapshot {
        path: path.to_string_lossy().to_string(),
        sha256: checksum::sha256_bytes(content.as_bytes()),
        content,
    }))
}

pub async fn profile_path_lossy() -> Option<PathBuf> {
    match profile_path().await {
        Ok(path) => path,
        Err(error) => {
            eprintln!("warning: PowerShell profile path probe failed: {error:#}");
            None
        }
    }
}

/// Reads a User-scope environment variable via the .NET API (the registry
/// value, not this process's inherited copy). `None` if unset or unreadable.
pub async fn get_user_env_var(name: &str) -> Result<Option<String>> {
    let Some(exe) = executable() else {
        anyhow::bail!("PowerShell not found; cannot read environment variable '{name}'");
    };
    let script = format!("[Environment]::GetEnvironmentVariable({name:?}, 'User')");
    let output = process::capture(&exe, &["-NoProfile", "-Command", &script]).await?;
    if output.code != 0 {
        anyhow::bail!(
            "reading environment variable '{name}' failed: {}",
            output.stderr
        );
    }
    let value = output.stdout.trim();
    Ok(if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    })
}

pub async fn set_user_env_var(name: &str, value: &str) -> Result<()> {
    let Some(exe) = executable() else {
        anyhow::bail!("PowerShell not found; cannot set environment variable '{name}'");
    };
    // Uses the .NET API directly — no 1024-char truncation, takes effect in new
    // processes without requiring a shell restart (unlike setx).
    let script = format!("[Environment]::SetEnvironmentVariable({name:?}, {value:?}, 'User')");
    process::checked(&exe, &["-NoProfile", "-Command", &script]).await?;
    Ok(())
}

fn powershell_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(Path::new(&program_files).join(r"PowerShell\7\pwsh.exe"));
    }
    if let Ok(system_root) = std::env::var("SystemRoot") {
        candidates
            .push(Path::new(&system_root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
    }
    candidates
}
