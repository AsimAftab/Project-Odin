use std::env;
use std::path::PathBuf;

use anyhow::Result;

use crate::integrations::powershell;
use crate::models::environment::{
    EnvironmentScope, EnvironmentSnapshot, EnvironmentVariable, PathEntry, ProfileSnapshot,
};
use crate::utils::{checksum, paths};

pub async fn environment(include_machine_env: bool) -> Result<EnvironmentSnapshot> {
    let mut user_variables = Vec::new();
    for (name, value) in env::vars() {
        if should_snapshot_env_var(&name) {
            user_variables.push(EnvironmentVariable {
                name,
                value,
                scope: EnvironmentScope::Process,
            });
        }
    }
    user_variables.sort_by(|left, right| left.name.cmp(&right.name));

    let machine_variables = if include_machine_env {
        read_registry_environment("Machine")
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let path = env::var("PATH").unwrap_or_default();
    let path_entries = path
        .split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let expanded = expand_windows_env(entry);
            PathEntry {
                value: entry.to_string(),
                exists: PathBuf::from(expanded).exists(),
                source: EnvironmentScope::Process,
            }
        })
        .collect();

    Ok(EnvironmentSnapshot {
        user_variables,
        machine_variables,
        path_entries,
        powershell_profile: powershell::read_profile().await?,
        terminal_settings: terminal_settings().await?,
    })
}

async fn read_registry_environment(scope: &str) -> Result<Vec<EnvironmentVariable>> {
    let hive = if scope == "Machine" { "HKLM" } else { "HKCU" };
    let key = if scope == "Machine" {
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment"
    } else {
        "Environment"
    };
    let output =
        crate::integrations::process::capture("reg", &["query", &format!(r"{hive}\{key}")]).await?;
    if output.code != 0 {
        return Ok(Vec::new());
    }
    let env_scope = if scope == "Machine" {
        EnvironmentScope::Machine
    } else {
        EnvironmentScope::User
    };
    let vars = output
        .stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 || !parts[1].starts_with("REG_") {
                return None;
            }
            Some(EnvironmentVariable {
                name: parts[0].to_string(),
                value: parts[2..].join(" "),
                scope: env_scope.clone(),
            })
        })
        .collect();
    Ok(vars)
}

pub async fn terminal_settings() -> Result<Option<ProfileSnapshot>> {
    let base = paths::user_profile()?;
    let candidates = [
        base.join(r"AppData\Local\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json"),
        base.join(r"AppData\Local\Packages\Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe\LocalState\settings.json"),
    ];
    for path in candidates {
        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            return Ok(Some(ProfileSnapshot {
                path: path.to_string_lossy().to_string(),
                sha256: checksum::sha256_bytes(content.as_bytes()),
                content,
            }));
        }
    }
    Ok(None)
}

pub fn expand_windows_env(value: &str) -> String {
    let mut expanded = value.to_string();
    for (key, val) in env::vars() {
        expanded = expanded.replace(&format!("%{key}%"), &val);
    }
    expanded
}

fn should_snapshot_env_var(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower == "path" {
        return false;
    }

    let volatile_prefixes = [
        "__",
        "posh_",
        "prompt_",
        "wt_",
        "term_session",
        "vscode_",
        "npm_config_",
    ];
    let volatile_names = [
        "?",
        "errorlevel",
        "random",
        "sessionname",
        "temp",
        "tmp",
        "terminal_emulator",
    ];

    !volatile_prefixes
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        && !volatile_names.iter().any(|candidate| lower == *candidate)
}
