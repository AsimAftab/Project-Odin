//! End-to-end command-dispatch tests: run the real `odin` binary against an
//! isolated `--odin-dir` (via the ODIN_DIR env var) so nothing touches the
//! developer's ~/.odin. Restore tests only ever dry-run.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn odin(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("odin").expect("binary builds");
    cmd.env("ODIN_DIR", dir);
    // Keep output deterministic for assertions.
    cmd.env("NO_COLOR", "1");
    cmd
}

/// Seeds a minimal but complete vault (the five snapshot files) so commands
/// that read the last snapshot have something to chew on.
fn seed_vault(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("packages.json"),
        serde_json::json!({
            "packages": [
                {
                    "id": "Test.Tool",
                    "name": "Test Tool",
                    "version": "1.0",
                    "source": "manual",
                    "install_command": null
                }
            ]
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        dir.join("env.json"),
        serde_json::json!({
            "user_variables": [
                { "name": "ODIN_TEST_VAR", "value": "1", "scope": "user" }
            ],
            "machine_variables": [],
            "path_entries": [],
            "powershell_profile": null,
            "terminal_settings": null
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        dir.join("vscode_extensions.json"),
        serde_json::json!({ "extensions": [] }).to_string(),
    )
    .unwrap();
    fs::write(
        dir.join("git_config.json"),
        serde_json::json!({ "entries": [] }).to_string(),
    )
    .unwrap();
    fs::write(dir.join("machine.json"), serde_json::json!({}).to_string()).unwrap();
}

#[test]
fn help_lists_core_commands() {
    let tmp = TempDir::new().unwrap();
    odin(tmp.path()).arg("--help").assert().success().stdout(
        predicate::str::contains("snapshot")
            .and(predicate::str::contains("restore"))
            .and(predicate::str::contains("login"))
            .and(predicate::str::contains("diff")),
    );
}

#[test]
fn version_matches_cargo_manifest() {
    let tmp = TempDir::new().unwrap();
    odin(tmp.path())
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_subcommand_fails_with_usage() {
    let tmp = TempDir::new().unwrap();
    odin(tmp.path())
        .arg("not-a-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage").or(predicate::str::contains("usage")));
}

#[test]
fn workspace_is_created_on_first_run() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("odin-home");
    odin(&dir).args(["config", "show"]).assert().success();
    assert!(dir.join("config.yaml").exists(), "config.yaml scaffolded");
    assert!(dir.join("logs").exists(), "logs/ scaffolded");
}

#[test]
fn restore_without_snapshot_fails_cleanly() {
    let tmp = TempDir::new().unwrap();
    odin(tmp.path())
        .arg("restore")
        .assert()
        .failure()
        .stderr(predicate::str::contains("snapshot"));
}

#[test]
fn restore_dry_run_emits_plan_json_and_touches_nothing() {
    let tmp = TempDir::new().unwrap();
    // First run scaffolds the workspace + config, then we seed the vault.
    odin(tmp.path()).args(["config", "show"]).assert().success();
    seed_vault(tmp.path());

    let output = odin(tmp.path())
        .args(["restore", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON report");

    assert_eq!(report["applied"], false, "dry-run must not apply");
    let sections: Vec<&str> = report["plan"]["sections"]
        .as_array()
        .expect("sections array")
        .iter()
        .map(|s| s["section"].as_str().unwrap())
        .collect();
    for expected in [
        "packages",
        "extensions",
        "git",
        "env",
        "path",
        "terminal",
        "ps-profile",
    ] {
        assert!(
            sections.contains(&expected),
            "plan lists section {expected}"
        );
    }
    // The manual-source test package has no install command.
    assert_eq!(
        report["plan"]["packages"][0]["action"],
        "no_install_command"
    );
    // Dry-run writes no restore report log.
    let logs: Vec<_> = fs::read_dir(tmp.path().join("logs"))
        .map(|d| d.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(logs.is_empty(), "dry-run must not write logs");
}

#[test]
fn restore_only_rejects_unknown_section() {
    let tmp = TempDir::new().unwrap();
    odin(tmp.path())
        .args(["restore", "--only", "bogus"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("bogus"));
}

#[test]
fn history_on_fresh_workspace_succeeds() {
    let tmp = TempDir::new().unwrap();
    odin(tmp.path()).arg("history").assert().success();
}
