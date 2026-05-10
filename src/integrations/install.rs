use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::integrations::process;

#[derive(Debug, Clone)]
pub struct InstallStatus {
    pub current_executable: PathBuf,
    pub current_directory: PathBuf,
    pub user_install_dir: PathBuf,
    pub machine_install_dir: PathBuf,
    pub process_path_entries: Vec<String>,
    pub process_has_current_directory: bool,
    pub user_path_has_user_install_dir: bool,
    pub machine_path_has_machine_install_dir: bool,
}

pub fn user_install_dir() -> Result<PathBuf> {
    let base =
        std::env::var("LOCALAPPDATA").context("LOCALAPPDATA environment variable is missing")?;
    Ok(PathBuf::from(base).join("Odin").join("bin"))
}

pub fn machine_install_dir() -> Result<PathBuf> {
    let base =
        std::env::var("ProgramFiles").context("ProgramFiles environment variable is missing")?;
    Ok(PathBuf::from(base).join("Odin"))
}

pub async fn collect_status() -> Result<InstallStatus> {
    let current_executable =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let current_directory = current_executable
        .parent()
        .map(Path::to_path_buf)
        .context("current executable has no parent directory")?;
    let user_install_dir = user_install_dir()?;
    let machine_install_dir = machine_install_dir()?;

    let process_path_entries = split_path_entries(&std::env::var("PATH").unwrap_or_default());
    let user_path_entries = split_path_entries(&read_registry_path("HKCU\\Environment").await?);
    let machine_path_entries = split_path_entries(
        &read_registry_path(
            "HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
        )
        .await?,
    );

    Ok(InstallStatus {
        process_has_current_directory: contains_dir(&process_path_entries, &current_directory),
        user_path_has_user_install_dir: contains_dir(&user_path_entries, &user_install_dir),
        machine_path_has_machine_install_dir: contains_dir(
            &machine_path_entries,
            &machine_install_dir,
        ),
        current_executable,
        current_directory,
        user_install_dir,
        machine_install_dir,
        process_path_entries,
    })
}

pub fn path_duplicates(entries: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    for entry in entries {
        let normalized = normalize_path(entry);
        if !seen.insert(normalized) {
            duplicates.push(entry.clone());
        }
    }
    duplicates
}

pub fn contains_dir(entries: &[String], dir: &Path) -> bool {
    let needle = normalize_path(dir.to_string_lossy().as_ref());
    entries.iter().any(|entry| normalize_path(entry) == needle)
}

pub fn odin_path_entries(entries: &[String]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| Path::new(entry).join("odin.exe").exists())
        .cloned()
        .collect()
}

fn split_path_entries(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('\\')
        .replace('/', "\\")
        .to_ascii_lowercase()
}

async fn read_registry_path(key: &str) -> Result<String> {
    let output = process::capture("reg", &["query", key, "/v", "Path"]).await?;
    if output.code != 0 {
        return Ok(String::new());
    }
    for line in output.stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with("path") {
            continue;
        }
        let parts = trimmed
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if parts.len() < 3 {
            continue;
        }
        return Ok(parts[2..].join(" "));
    }
    Ok(String::new())
}

pub async fn add_to_user_path(dir: &Path) -> Result<()> {
    let dir_str = dir.to_string_lossy();
    let script = format!(
        r#"
$dir = '{}'
$oldPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$entries = $oldPath.Split(';') | ForEach-Object {{ $_.Trim() }} | Where-Object {{ $_ -ne '' }}
if ($entries -notcontains $dir) {{
    $newPath = ($entries + $dir) -join ';'
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    
    # Broadcast change
    $signature = @'
[DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
    $type = Add-Type -MemberDefinition $signature -Name "Win32Environment" -Namespace "Odin.Internal" -PassThru
    $result = [UIntPtr]::Zero
    $type::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, "Environment", 2, 5000, [ref]$result) | Out-Null
}}
"#,
        dir_str
    );

    let pwsh = super::powershell::executable().context("PowerShell is required to update PATH")?;
    process::checked(&pwsh, &["-NoProfile", "-Command", &script]).await?;
    Ok(())
}
