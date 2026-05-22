use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;

use crate::asgard::profile::{StartupApp, WindowState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub fn build_spec(app: &StartupApp) -> LaunchSpec {
    let mut args: Vec<String> = vec!["/C".into(), "start".into(), String::new()];
    match app.window {
        WindowState::Normal => {}
        WindowState::Minimized => args.push("/MIN".into()),
        WindowState::Maximized => args.push("/MAX".into()),
    }
    if let Some(cwd) = app.cwd.as_deref().filter(|s| !s.is_empty()) {
        args.push("/D".into());
        args.push(cwd.into());
    }
    args.push(app.command.clone());
    for a in &app.args {
        args.push(a.clone());
    }
    LaunchSpec {
        program: "cmd.exe".into(),
        args,
    }
}

pub fn build_url_spec(url: &str) -> LaunchSpec {
    LaunchSpec {
        program: "cmd.exe".into(),
        args: vec!["/C".into(), "start".into(), String::new(), url.into()],
    }
}

pub fn launch(app: &StartupApp, env: &BTreeMap<String, String>) -> Result<()> {
    let spec = build_spec(app);
    spawn_detached(&spec, env).with_context(|| format!("failed to launch app `{}`", app.name))?;
    Ok(())
}

pub fn launch_url(url: &str) -> Result<()> {
    let spec = build_url_spec(url);
    spawn_detached(&spec, &BTreeMap::new())
        .with_context(|| format!("failed to open URL `{url}`"))?;
    Ok(())
}

fn spawn_detached(spec: &LaunchSpec, env: &BTreeMap<String, String>) -> Result<()> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.spawn()
        .with_context(|| format!("failed to spawn `{}`", spec.program))?;
    Ok(())
}

pub fn looks_like_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn open_vscode_workspace(workspace: &Path) -> Result<()> {
    let workspace_str = workspace.to_string_lossy().to_string();
    let spec = LaunchSpec {
        program: "cmd.exe".into(),
        args: vec![
            "/C".into(),
            "start".into(),
            String::new(),
            "code".into(),
            workspace_str.clone(),
        ],
    };
    spawn_detached(&spec, &BTreeMap::new())
        .with_context(|| format!("failed to open VS Code workspace `{}`", workspace.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(window: WindowState, cwd: Option<&str>, command: &str, args: &[&str]) -> StartupApp {
        StartupApp {
            name: "x".into(),
            command: command.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(|s| s.to_string()),
            window,
            layout: None,
        }
    }

    #[test]
    fn url_detection() {
        assert!(looks_like_url("https://example.com"));
        assert!(looks_like_url("HTTP://example.com"));
        assert!(!looks_like_url("notepad"));
        assert!(!looks_like_url("C:\\Windows\\notepad.exe"));
    }

    #[test]
    fn build_spec_normal_no_cwd() {
        let s = build_spec(&app(WindowState::Normal, None, "notepad", &[]));
        assert_eq!(s.program, "cmd.exe");
        assert_eq!(s.args, vec!["/C", "start", "", "notepad"]);
    }

    #[test]
    fn build_spec_minimized_with_cwd_and_args() {
        let s = build_spec(&app(
            WindowState::Minimized,
            Some("C:\\repos"),
            "code",
            &["."],
        ));
        assert_eq!(
            s.args,
            vec!["/C", "start", "", "/MIN", "/D", "C:\\repos", "code", "."]
        );
    }

    #[test]
    fn build_spec_maximized() {
        let s = build_spec(&app(WindowState::Maximized, None, "wt.exe", &["new-tab"]));
        assert_eq!(s.args, vec!["/C", "start", "", "/MAX", "wt.exe", "new-tab"]);
    }

    #[test]
    fn build_url_spec_uses_empty_title() {
        let s = build_url_spec("https://example.com");
        assert_eq!(s.args, vec!["/C", "start", "", "https://example.com"]);
    }

    #[test]
    fn empty_cwd_is_skipped() {
        let s = build_spec(&app(WindowState::Normal, Some(""), "notepad", &[]));
        assert!(!s.args.iter().any(|a| a == "/D"));
    }
}
