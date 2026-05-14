use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sysinfo::System;
use tokio::process::Command;

use crate::core::errors::OdinError;
use crate::models::process::{PortInfo, ProcessInfo};

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

pub async fn get_listening_ports() -> Result<Vec<PortInfo>> {
    let output = capture("netstat", &["-ano"]).await?;
    let mut sys = System::new_all();
    sys.refresh_all();

    // Build a map of PID -> process name for fast lookup
    let pid_to_name: HashMap<u32, String> = sys
        .processes()
        .iter()
        .map(|(pid, process)| (pid.as_u32(), process.name().to_string()))
        .collect();

    let mut ports = Vec::new();

    for line in output.stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let protocol = parts[0];
        if !protocol.eq_ignore_ascii_case("tcp") && !protocol.eq_ignore_ascii_case("udp") {
            continue;
        }

        if let Some(local_addr) = parts.get(1) {
            if let Some(port_str) = local_addr.split(':').next_back() {
                if let Ok(port) = port_str.parse::<u16>() {
                    if let Some(pid_str) = parts.last() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            let process_name = pid_to_name
                                .get(&pid)
                                .cloned()
                                .unwrap_or_else(|| format!("PID-{}", pid));

                            ports.push(PortInfo {
                                port,
                                protocol: protocol.to_uppercase(),
                                pid,
                                process_name,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(ports)
}

pub async fn find_process_by_port(port: u16) -> Result<Option<ProcessInfo>> {
    let ports = get_listening_ports().await?;
    if let Some(port_info) = ports.iter().find(|p| p.port == port) {
        let process_info = ProcessInfo {
            pid: port_info.pid,
            name: port_info.process_name.clone(),
            memory_mb: 0.0,
            cpu_percent: 0.0,
            status: "running".to_string(),
        };
        Ok(Some(process_info))
    } else {
        Ok(None)
    }
}

pub async fn kill_process_by_id(pid: u32) -> Result<String> {
    let output = capture("taskkill", &["/PID", &pid.to_string(), "/F"]).await?;
    if output.code == 0 {
        Ok(format!("Process {} killed successfully", pid))
    } else {
        Err(anyhow::anyhow!(
            "Failed to kill process {}: {}",
            pid,
            output.stderr
        ))
    }
}
