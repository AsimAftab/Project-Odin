use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::integrations::{git_cli, powershell, process, vscode as vscode_integration};
use crate::models::config::RestoreConfig;
use crate::models::environment::EnvironmentSnapshot;
use crate::models::git::GitConfigSnapshot;
use crate::models::package::{InstalledPackage, PackageManager, PackageSnapshot};
use crate::models::vscode::VsCodeExtensionsSnapshot;
use crate::services::storage::SnapshotStore;

pub struct RestoreService {
    store: SnapshotStore,
    config: RestoreConfig,
}

impl RestoreService {
    pub fn new(store: SnapshotStore, config: RestoreConfig) -> Self {
        Self { store, config }
    }

    pub async fn restore(&self, apply: bool, continue_on_error: bool) -> Result<()> {
        let packages = self.store.read_packages().await?;
        let environment = self.store.read_environment().await?;
        let vscode = self.store.read_vscode().await?;
        let git = self.store.read_git().await?;
        run_restore(
            &self.config,
            &packages,
            &environment,
            &vscode,
            &git,
            apply,
            continue_on_error,
        )
        .await
    }

    pub async fn restore_from_id(
        &self,
        snapshot_id: &str,
        apply: bool,
        continue_on_error: bool,
    ) -> Result<()> {
        let history_root = self.store.root().join("history").join(snapshot_id);
        if !history_root.exists() {
            anyhow::bail!(
                "Historical snapshot files not found at {} — was this snapshot captured before per-id history was added? Run `odin snapshot` again to create a restorable snapshot.",
                history_root.display()
            );
        }
        let history_store = SnapshotStore::new(history_root);
        let packages = history_store
            .read_packages()
            .await
            .with_context(|| format!("reading packages for snapshot {}", snapshot_id))?;
        let environment = history_store
            .read_environment()
            .await
            .with_context(|| format!("reading environment for snapshot {}", snapshot_id))?;
        let vscode = history_store
            .read_vscode()
            .await
            .with_context(|| format!("reading vscode extensions for snapshot {}", snapshot_id))?;
        let git = history_store
            .read_git()
            .await
            .with_context(|| format!("reading git config for snapshot {}", snapshot_id))?;
        run_restore(
            &self.config,
            &packages,
            &environment,
            &vscode,
            &git,
            apply,
            continue_on_error,
        )
        .await
    }
}

/// True if a package's source manager is enabled in `RestoreConfig`. Alias-aware
/// (`choco` == `chocolatey`). `Manual`/`Unknown` packages can't be attributed to
/// a manager, so they're always allowed (they only install if they carry a
/// command anyway).
pub fn source_enabled(source: &PackageManager, managers: &[String]) -> bool {
    let aliases: &[&str] = match source {
        PackageManager::Winget => &["winget"],
        PackageManager::Chocolatey => &["chocolatey", "choco"],
        PackageManager::Scoop => &["scoop"],
        PackageManager::Npm => &["npm"],
        PackageManager::Pip => &["pip"],
        PackageManager::Cargo => &["cargo"],
        PackageManager::Pipx => &["pipx"],
        PackageManager::Pnpm => &["pnpm"],
        PackageManager::Yarn => &["yarn"],
        PackageManager::DotnetTool => &["dotnet", "dotnet-tool", "dotnettool"],
        PackageManager::Go => &["go"],
        PackageManager::Uv => &["uv"],
        PackageManager::Manual | PackageManager::Unknown => return true,
    };
    managers
        .iter()
        .any(|m| aliases.iter().any(|a| m.eq_ignore_ascii_case(a)))
}

async fn run_restore(
    config: &RestoreConfig,
    packages: &PackageSnapshot,
    environment: &EnvironmentSnapshot,
    vscode: &VsCodeExtensionsSnapshot,
    git: &GitConfigSnapshot,
    apply: bool,
    continue_on_error: bool,
) -> Result<()> {
    let current = crate::integrations::package_managers::list_packages().await?;
    restore_packages(config, packages, &current, apply, continue_on_error).await?;

    if config.restore_vscode_extensions {
        restore_vscode(vscode, apply).await?;
    } else {
        println!(
            "  {}  VS Code extensions skipped (disabled by config)",
            "·".dimmed()
        );
    }

    if config.restore_git_config {
        restore_git(git, apply).await?;
    } else {
        println!(
            "  {}  git config skipped (disabled by config)",
            "·".dimmed()
        );
    }

    apply_environment(config, environment, apply).await?;
    Ok(())
}

async fn restore_packages(
    config: &RestoreConfig,
    packages: &PackageSnapshot,
    current: &PackageSnapshot,
    apply: bool,
    continue_on_error: bool,
) -> Result<()> {
    // Drop packages whose source manager is disabled in config.
    let selected: Vec<&InstalledPackage> = packages
        .packages
        .iter()
        .filter(|p| source_enabled(&p.source, &config.package_managers))
        .collect();
    let skipped = packages.packages.len() - selected.len();
    if skipped > 0 {
        println!(
            "  {}  {} package(s) skipped (manager disabled by config)",
            "·".dimmed(),
            skipped.to_string().cyan()
        );
    }

    let bar = ProgressBar::new(selected.len() as u64);
    bar.set_style(ProgressStyle::with_template(
        "  {spinner:.yellow} [{elapsed_precise}] [{bar:32.yellow/blue}] {pos}/{len} {msg}",
    )?);

    for package in selected {
        bar.set_message(package.id.clone());
        if installed(package, &current.packages) {
            println!("  {}  {}", "·".green(), package.id.dimmed());
            bar.inc(1);
            continue;
        }
        let Some(command) = &package.install_command else {
            println!(
                "  {}  {} has no install command",
                "!".yellow().bold(),
                package.id.cyan()
            );
            bar.inc(1);
            continue;
        };
        println!(
            "  {}  {}",
            if apply {
                "→".bright_blue().bold()
            } else {
                "·".yellow().bold()
            },
            command.dimmed()
        );
        if apply {
            let (program, args) = split_command(command);
            let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
            let result = process::checked(&program, &arg_refs).await;
            if let Err(error) = result {
                if continue_on_error {
                    eprintln!("  {}  {error:#}", "✗".red().bold());
                } else {
                    return Err(error);
                }
            }
        }
        bar.inc(1);
    }
    bar.finish_and_clear();
    Ok(())
}

async fn restore_vscode(vscode: &VsCodeExtensionsSnapshot, apply: bool) -> Result<()> {
    for extension in &vscode.extensions {
        let command = format!("code --install-extension {}", extension.identifier);
        println!(
            "  {}  {}",
            if apply {
                "→".bright_blue().bold()
            } else {
                "·".yellow().bold()
            },
            command.dimmed()
        );
        if apply {
            if let Some(code) = vscode_integration::executable() {
                process::checked(&code, &["--install-extension", &extension.identifier]).await?;
            }
        }
    }
    Ok(())
}

async fn restore_git(git: &GitConfigSnapshot, apply: bool) -> Result<()> {
    for entry in &git.entries {
        println!(
            "  {}  git config --global {} <value>",
            if apply {
                "→".bright_blue().bold()
            } else {
                "·".yellow().bold()
            },
            entry.key.cyan()
        );
        if apply {
            if let Some(git_bin) = git_cli::executable() {
                process::checked(&git_bin, &["config", "--global", &entry.key, &entry.value])
                    .await?;
            }
        }
    }
    Ok(())
}

fn installed(package: &InstalledPackage, current: &[InstalledPackage]) -> bool {
    current.iter().any(|candidate| {
        candidate.source == package.source && candidate.id.eq_ignore_ascii_case(&package.id)
    })
}

fn split_command(command: &str) -> (String, Vec<String>) {
    let mut parts = command.split_whitespace();
    let program = parts.next().unwrap_or_default().to_string();
    (program, parts.map(ToOwned::to_owned).collect())
}

async fn apply_environment(
    config: &RestoreConfig,
    environment: &EnvironmentSnapshot,
    apply: bool,
) -> Result<()> {
    // User environment variables (excluding PATH, handled separately).
    if config.restore_user_environment {
        if apply {
            let mut applied = 0usize;
            for variable in &environment.user_variables {
                if variable.name.eq_ignore_ascii_case("PATH") {
                    continue;
                }
                powershell::set_user_env_var(&variable.name, &variable.value).await?;
                applied += 1;
            }
            if applied > 0 {
                println!(
                    "  {}  carved {} rune(s) into the environment",
                    "✓".green().bold(),
                    applied.to_string().cyan().bold()
                );
            }
        } else {
            let count = environment
                .user_variables
                .iter()
                .filter(|v| !v.name.eq_ignore_ascii_case("PATH"))
                .count();
            println!(
                "  {}  would restore {} rune(s)",
                "·".yellow().bold(),
                count.to_string().cyan().bold()
            );
        }
    } else {
        println!(
            "  {}  environment variables skipped (disabled by config)",
            "·".dimmed()
        );
    }

    // PATH.
    if config.restore_path {
        let path_value = environment
            .path_entries
            .iter()
            .map(|entry| entry.value.as_str())
            .collect::<Vec<_>>()
            .join(";");
        if !path_value.is_empty() {
            if apply {
                powershell::set_user_env_var("Path", &path_value).await?;
                println!(
                    "  {}  PATH bound — {} entries",
                    "✓".green().bold(),
                    environment.path_entries.len().to_string().cyan().bold()
                );
            } else {
                println!(
                    "  {}  would bind PATH — {} entries",
                    "·".yellow().bold(),
                    environment.path_entries.len().to_string().cyan().bold()
                );
            }
        }
    } else {
        println!("  {}  PATH skipped (disabled by config)", "·".dimmed());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn managers(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn enabled_when_manager_listed() {
        let m = managers(&["winget", "scoop"]);
        assert!(source_enabled(&PackageManager::Winget, &m));
        assert!(source_enabled(&PackageManager::Scoop, &m));
        assert!(!source_enabled(&PackageManager::Npm, &m));
    }

    #[test]
    fn choco_alias_matches_chocolatey() {
        assert!(source_enabled(
            &PackageManager::Chocolatey,
            &managers(&["choco"])
        ));
        assert!(source_enabled(
            &PackageManager::Chocolatey,
            &managers(&["chocolatey"])
        ));
        assert!(!source_enabled(
            &PackageManager::Chocolatey,
            &managers(&["scoop"])
        ));
    }

    #[test]
    fn manual_and_unknown_always_enabled() {
        let empty: Vec<String> = vec![];
        assert!(source_enabled(&PackageManager::Manual, &empty));
        assert!(source_enabled(&PackageManager::Unknown, &empty));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(source_enabled(
            &PackageManager::Winget,
            &managers(&["WinGet"])
        ));
    }
}
