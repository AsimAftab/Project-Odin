use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};

use crate::asgard::profile::{StartupApp, WindowState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
}

/// How a startup app will be launched. Computed by [`plan_launch`] so the
/// caller can know up-front whether the launch will yield an owned PID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchPlan {
    /// Spawn the program directly (or via `cmd.exe /C` for `.cmd`/`.bat`
    /// shims when `via_cmd` is true). We own the resulting PID.
    DirectExe {
        program: PathBuf,
        args: Vec<String>,
        via_cmd: bool,
    },
    /// UWP `shell:` AUMID launched via `explorer.exe <arg>` — a single
    /// argument, so `!` in AUMIDs never hits cmd quoting hazards. PID not owned.
    Explorer { arg: String },
    /// Fall back to `cmd /C start` (URLs, `.lnk`/`.url`, documents,
    /// unresolvable commands). PID not owned.
    ShellStart(LaunchSpec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub pid: Option<u32>,
}

/// True if the command is a UWP `shell:` AUMID (case-insensitive).
pub fn is_shell_command(s: &str) -> bool {
    s.get(..6).is_some_and(|p| p.eq_ignore_ascii_case("shell:"))
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Decide how to launch `app`, using `resolve` to map bare command names to
/// paths (production uses `which`).
pub fn plan_launch_with_resolver(
    app: &StartupApp,
    resolve: impl Fn(&str) -> Option<PathBuf>,
) -> LaunchPlan {
    if looks_like_url(&app.command) {
        return LaunchPlan::ShellStart(build_spec(app));
    }

    if is_shell_command(&app.command) {
        return LaunchPlan::Explorer {
            arg: app.command.clone(),
        };
    }

    let as_path = Path::new(&app.command);
    if as_path.is_absolute() && has_extension(as_path, "exe") && as_path.exists() {
        return LaunchPlan::DirectExe {
            program: as_path.to_path_buf(),
            args: app.args.clone(),
            via_cmd: false,
        };
    }

    if let Some(resolved) = resolve(&app.command) {
        if has_extension(&resolved, "exe") {
            return LaunchPlan::DirectExe {
                program: resolved,
                args: app.args.clone(),
                via_cmd: false,
            };
        }
        if has_extension(&resolved, "cmd") || has_extension(&resolved, "bat") {
            // Run the shim through cmd.exe WITHOUT `start`, so the cmd process
            // we spawn is the parent of the real app — descendant matching in
            // the window manager can then find the real window.
            let mut args = vec!["/C".to_string(), resolved.to_string_lossy().to_string()];
            args.extend(app.args.iter().cloned());
            return LaunchPlan::DirectExe {
                program: PathBuf::from("cmd.exe"),
                args,
                via_cmd: true,
            };
        }
    }

    // .lnk/.url/documents/unresolvable — let the shell figure it out.
    LaunchPlan::ShellStart(build_spec(app))
}

pub fn plan_launch(app: &StartupApp) -> LaunchPlan {
    plan_launch_with_resolver(app, |c| which::which(c).ok())
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

pub async fn launch(app: &StartupApp, env: &BTreeMap<String, String>) -> Result<LaunchOutcome> {
    match plan_launch(app) {
        LaunchPlan::DirectExe {
            program,
            args,
            via_cmd: _,
        } => {
            let pid = spawn_direct(&program, &args, app.cwd.as_deref(), env)
                .with_context(|| format!("failed to launch app `{}`", app.name))?;
            Ok(LaunchOutcome { pid: Some(pid) })
        }
        LaunchPlan::Explorer { arg } => {
            spawn_direct(
                Path::new("explorer.exe"),
                std::slice::from_ref(&arg),
                None,
                env,
            )
            .with_context(|| format!("failed to launch app `{}`", app.name))?;
            Ok(LaunchOutcome { pid: None })
        }
        LaunchPlan::ShellStart(spec) => {
            spawn_detached(&spec, env)
                .with_context(|| format!("failed to launch app `{}`", app.name))?;
            Ok(LaunchOutcome { pid: None })
        }
    }
}

pub fn launch_url(url: &str) -> Result<()> {
    let spec = build_url_spec(url);
    spawn_detached(&spec, &BTreeMap::new())
        .with_context(|| format!("failed to open URL `{url}`"))?;
    Ok(())
}

/// Spawn a process directly with std, detached from our console. The `Child`
/// is dropped — std's `Command` does not kill on drop — but its PID is
/// returned so the window manager can match windows by process.
fn spawn_direct(
    program: &Path,
    args: &[String],
    cwd: Option<&str>,
    env: &BTreeMap<String, String>,
) -> Result<u32> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::System::Threading::{CREATE_NEW_CONSOLE, CREATE_NEW_PROCESS_GROUP};

    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .creation_flags(CREATE_NEW_PROCESS_GROUP.0 | CREATE_NEW_CONSOLE.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = cwd.filter(|s| !s.is_empty()) {
        cmd.current_dir(cwd);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn `{}`", program.display()))?;
    Ok(child.id())
}

fn spawn_detached(spec: &LaunchSpec, env: &BTreeMap<String, String>) -> Result<()> {
    let mut cmd = tokio::process::Command::new(&spec.program);
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

pub async fn open_vscode_workspace(workspace: &Path) -> Result<LaunchOutcome> {
    let app = StartupApp {
        name: "vscode".into(),
        command: "code".into(),
        args: vec![workspace.to_string_lossy().to_string()],
        cwd: None,
        window: WindowState::Normal,
        layout: None,
    };
    launch(&app, &BTreeMap::new())
        .await
        .with_context(|| format!("failed to open VS Code workspace `{}`", workspace.display()))
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
    fn plan_bare_exe_resolves_to_direct_exe() {
        let plan =
            plan_launch_with_resolver(&app(WindowState::Normal, None, "notepad", &[]), |c| {
                assert_eq!(c, "notepad");
                Some(PathBuf::from(r"C:\Windows\notepad.exe"))
            });
        assert_eq!(
            plan,
            LaunchPlan::DirectExe {
                program: PathBuf::from(r"C:\Windows\notepad.exe"),
                args: vec![],
                via_cmd: false,
            }
        );
    }

    #[test]
    fn plan_cmd_shim_goes_via_cmd_without_start() {
        let plan =
            plan_launch_with_resolver(&app(WindowState::Normal, None, "code", &["."]), |_| {
                Some(PathBuf::from(r"C:\Program Files\VS Code\bin\code.cmd"))
            });
        assert_eq!(
            plan,
            LaunchPlan::DirectExe {
                program: PathBuf::from("cmd.exe"),
                args: vec![
                    "/C".into(),
                    r"C:\Program Files\VS Code\bin\code.cmd".into(),
                    ".".into(),
                ],
                via_cmd: true,
            }
        );
    }

    #[test]
    fn plan_shell_aumid_goes_via_explorer_verbatim() {
        let cmd = "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";
        let plan = plan_launch_with_resolver(&app(WindowState::Normal, None, cmd, &[]), |_| {
            panic!("resolver must not be consulted for shell: commands")
        });
        assert_eq!(
            plan,
            LaunchPlan::Explorer {
                arg: cmd.to_string()
            }
        );
    }

    #[test]
    fn plan_url_goes_via_shell_start() {
        let plan = plan_launch_with_resolver(
            &app(WindowState::Normal, None, "https://example.com", &[]),
            |_| panic!("resolver must not be consulted for URLs"),
        );
        assert_eq!(
            plan,
            LaunchPlan::ShellStart(LaunchSpec {
                program: "cmd.exe".into(),
                args: vec![
                    "/C".into(),
                    "start".into(),
                    String::new(),
                    "https://example.com".into(),
                ],
            })
        );
    }

    #[test]
    fn plan_unresolvable_lnk_falls_back_to_shell_start_with_flags() {
        let plan = plan_launch_with_resolver(
            &app(WindowState::Minimized, Some("C:\\repos"), "foo.lnk", &[]),
            |_| None,
        );
        assert_eq!(
            plan,
            LaunchPlan::ShellStart(LaunchSpec {
                program: "cmd.exe".into(),
                args: vec![
                    "/C".into(),
                    "start".into(),
                    String::new(),
                    "/MIN".into(),
                    "/D".into(),
                    "C:\\repos".into(),
                    "foo.lnk".into(),
                ],
            })
        );

        let plan =
            plan_launch_with_resolver(&app(WindowState::Maximized, None, "foo.lnk", &[]), |_| None);
        let LaunchPlan::ShellStart(spec) = plan else {
            panic!("expected ShellStart");
        };
        assert_eq!(spec.args, vec!["/C", "start", "", "/MAX", "foo.lnk"]);
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

    #[test]
    fn shell_command_detection_is_case_insensitive() {
        assert!(is_shell_command("shell:AppsFolder\\X!App"));
        assert!(is_shell_command("SHELL:AppsFolder\\X!App"));
        assert!(!is_shell_command("notepad"));
        assert!(!is_shell_command("shel"));
    }
}
