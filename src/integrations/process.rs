use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::process::Command;

use crate::core::errors::OdinError;

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

pub fn command_exists(command: &str) -> bool {
    which::which(command).is_ok()
}

pub async fn capture(command: &str, args: &[&str]) -> Result<CommandOutput> {
    let mut process = command_for(command, args);
    let output = process
        .output()
        .await
        .with_context(|| format!("failed to start `{}`", display_command(command, args)))?;
    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        code: output.status.code().unwrap_or(-1),
    })
}

fn command_for(command: &str, args: &[&str]) -> Command {
    let resolved = resolve_command(command).unwrap_or_else(|| PathBuf::from(command));
    let resolved_text = resolved.to_string_lossy().to_string();

    if cfg!(windows) && is_windows_command_script(&resolved) {
        let mut process = Command::new("cmd.exe");
        process.arg("/C").arg(&resolved_text).args(args);
        process
    } else {
        let mut process = Command::new(&resolved_text);
        process.args(args);
        process
    }
}

fn resolve_command(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() || command.contains('\\') || command.contains('/') {
        return path.exists().then(|| path.to_path_buf());
    }
    which::which(command).ok()
}

fn is_windows_command_script(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
        .unwrap_or(false)
}

pub async fn checked(command: &str, args: &[&str]) -> Result<CommandOutput> {
    let output = capture(command, args).await?;
    if output.code == 0 {
        Ok(output)
    } else {
        Err(OdinError::CommandFailed {
            command: format!("{command} {}", args.join(" ")),
            code: output.code,
            stderr: output.stderr,
        }
        .into())
    }
}

fn display_command(command: &str, args: &[&str]) -> String {
    if args.is_empty() {
        command.to_string()
    } else {
        format!("{command} {}", args.join(" "))
    }
}
