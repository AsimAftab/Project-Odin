use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::integrations::process;

/// A Start-menu-launchable application as reported by `Get-StartApps`.
///
/// `app_id` is what we hand back to `cmd /C start "" shell:AppsFolder\<app_id>`
/// — that path works for both UWP packages and classic shortcut entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartApp {
    pub name: String,
    pub app_id: String,
}

#[derive(Debug, Deserialize)]
struct RawStartApp {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "AppID")]
    app_id: String,
}

const PS_SCRIPT: &str = "Get-StartApps | ConvertTo-Json -Compress";

/// Enumerate all Start-menu apps via PowerShell. Returns sorted, deduped.
pub async fn list_installed() -> Result<Vec<StartApp>> {
    let output = process::capture(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            PS_SCRIPT,
        ],
    )
    .await
    .context("failed to invoke powershell.exe")?;

    if output.code != 0 {
        return Err(anyhow!(
            "Get-StartApps failed (code {}): {}",
            output.code,
            output.stderr
        ));
    }
    parse_json(&output.stdout)
}

fn parse_json(stdout: &str) -> Result<Vec<StartApp>> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    // Get-StartApps returns a JSON object for a single result, an array for many.
    let raw: Vec<RawStartApp> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).context("parsing Get-StartApps JSON array")?
    } else {
        let single: RawStartApp =
            serde_json::from_str(trimmed).context("parsing Get-StartApps JSON object")?;
        vec![single]
    };

    let mut apps: Vec<StartApp> = raw
        .into_iter()
        .filter(|r| !r.name.trim().is_empty() && !r.app_id.trim().is_empty())
        .map(|r| StartApp {
            name: r.name,
            app_id: r.app_id,
        })
        .collect();

    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.app_id.eq_ignore_ascii_case(&b.app_id));
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_array() {
        let json = r#"[
            {"Name":"Visual Studio Code","AppID":"C:\\Users\\u\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe"},
            {"Name":"Calculator","AppID":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"}
        ]"#;
        let apps = parse_json(json).unwrap();
        assert_eq!(apps.len(), 2);
        // sorted by lowercase name
        assert_eq!(apps[0].name, "Calculator");
        assert_eq!(apps[1].name, "Visual Studio Code");
    }

    #[test]
    fn parse_single_object() {
        let json = r#"{"Name":"Notepad","AppID":"notepad.exe"}"#;
        let apps = parse_json(json).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].app_id, "notepad.exe");
    }

    #[test]
    fn parse_empty() {
        assert!(parse_json("").unwrap().is_empty());
        assert!(parse_json("   \n  ").unwrap().is_empty());
    }

    #[test]
    fn dedup_by_app_id_case_insensitive() {
        let json = r#"[
            {"Name":"Foo","AppID":"FOO!App"},
            {"Name":"Foo Dup","AppID":"foo!app"}
        ]"#;
        let apps = parse_json(json).unwrap();
        assert_eq!(apps.len(), 1);
    }

    #[test]
    fn skip_blank_entries() {
        let json = r#"[
            {"Name":"","AppID":"x"},
            {"Name":"Real","AppID":""},
            {"Name":"Keep","AppID":"keep!App"}
        ]"#;
        let apps = parse_json(json).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Keep");
    }
}
