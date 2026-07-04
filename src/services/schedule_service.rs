//! Windows Task Scheduler integration for recurring snapshots.
//!
//! Registers a per-user scheduled task (no admin/stored credentials) that runs
//! `odin snapshot [--push]` on an interval, via `schtasks.exe`. The argument
//! builders are pure so they can be unit-tested without touching the scheduler.

use anyhow::{Context, Result};

use crate::cli::ScheduleInterval;
use crate::integrations::process;

pub const TASK_NAME: &str = "OdinSnapshot";

/// Builds the `/TR` value: the command line the task runs. The exe path is
/// wrapped in quotes so a path with spaces survives Task Scheduler's re-parse.
pub fn build_task_run(exe: &str, push: bool) -> String {
    let mut cmd = format!("\"{exe}\" snapshot");
    if push {
        cmd.push_str(" --push");
    }
    cmd
}

/// Builds the full `schtasks /Create` argument list. `time` is HH:MM (24-hour),
/// used only for the daily schedule.
pub fn build_create_args(
    exe: &str,
    interval: ScheduleInterval,
    time: &str,
    push: bool,
) -> Vec<String> {
    let mut args = vec![
        "/Create".to_string(),
        "/TN".to_string(),
        TASK_NAME.to_string(),
        "/TR".to_string(),
        build_task_run(exe, push),
        "/SC".to_string(),
    ];
    match interval {
        ScheduleInterval::Daily => {
            args.push("DAILY".to_string());
            args.push("/ST".to_string());
            args.push(time.to_string());
        }
        ScheduleInterval::Hourly => {
            args.push("HOURLY".to_string());
            args.push("/MO".to_string());
            args.push("1".to_string());
        }
    }
    // /F overwrites an existing task without prompting.
    args.push("/F".to_string());
    args
}

fn current_exe() -> Result<String> {
    Ok(std::env::current_exe()
        .context("could not resolve the Odin executable path")?
        .to_string_lossy()
        .to_string())
}

pub async fn enable(interval: ScheduleInterval, time: &str, push: bool) -> Result<()> {
    let exe = current_exe()?;
    let args = build_create_args(&exe, interval, time, push);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = process::capture("schtasks", &arg_refs).await?;
    if output.code != 0 {
        anyhow::bail!(
            "schtasks failed to create the task: {}",
            if output.stderr.is_empty() {
                output.stdout
            } else {
                output.stderr
            }
        );
    }
    Ok(())
}

pub async fn disable() -> Result<()> {
    let output = process::capture("schtasks", &["/Delete", "/TN", TASK_NAME, "/F"]).await?;
    if output.code != 0 {
        anyhow::bail!(
            "schtasks failed to delete the task: {}",
            if output.stderr.is_empty() {
                output.stdout
            } else {
                output.stderr
            }
        );
    }
    Ok(())
}

/// Returns true if the task currently exists.
pub async fn status() -> Result<bool> {
    let output = process::capture("schtasks", &["/Query", "/TN", TASK_NAME, "/FO", "LIST"]).await?;
    Ok(output.code == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_run_quotes_exe_and_appends_push() {
        assert_eq!(
            build_task_run("C:\\Program Files\\Odin\\odin.exe", true),
            "\"C:\\Program Files\\Odin\\odin.exe\" snapshot --push"
        );
        assert_eq!(build_task_run("odin.exe", false), "\"odin.exe\" snapshot");
    }

    #[test]
    fn daily_args_include_time() {
        let args = build_create_args("odin.exe", ScheduleInterval::Daily, "08:30", false);
        assert!(args.iter().any(|a| a == "DAILY"));
        let st = args.iter().position(|a| a == "/ST").unwrap();
        assert_eq!(args[st + 1], "08:30");
        assert!(args.iter().any(|a| a == "/F"));
        assert_eq!(args[2], TASK_NAME);
    }

    #[test]
    fn hourly_args_use_modifier() {
        let args = build_create_args("odin.exe", ScheduleInterval::Hourly, "09:00", true);
        assert!(args.iter().any(|a| a == "HOURLY"));
        let mo = args.iter().position(|a| a == "/MO").unwrap();
        assert_eq!(args[mo + 1], "1");
        // Push flag is reflected in the /TR value.
        let tr = args.iter().position(|a| a == "/TR").unwrap();
        assert!(args[tr + 1].contains("--push"));
    }
}
