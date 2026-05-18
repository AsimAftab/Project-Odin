use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::integrations::{git_cli, process, vscode as vscode_integration};
use crate::models::environment::EnvironmentSnapshot;
use crate::models::git::GitConfigSnapshot;
use crate::models::package::{InstalledPackage, PackageSnapshot};
use crate::models::vscode::VsCodeExtensionsSnapshot;
use crate::services::storage::SnapshotStore;

pub struct RestoreService {
    store: SnapshotStore,
}

impl RestoreService {
    pub fn new(store: SnapshotStore) -> Self {
        Self { store }
    }

    pub async fn restore(&self, apply: bool, continue_on_error: bool) -> Result<()> {
        let packages = self.store.read_packages().await?;
        let environment = self.store.read_environment().await?;
        let vscode = self.store.read_vscode().await?;
        let git = self.store.read_git().await?;
        run_restore(
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

async fn run_restore(
    packages: &PackageSnapshot,
    environment: &EnvironmentSnapshot,
    vscode: &VsCodeExtensionsSnapshot,
    git: &GitConfigSnapshot,
    apply: bool,
    continue_on_error: bool,
) -> Result<()> {
    let current = crate::integrations::package_managers::list_packages().await?;
    restore_packages(packages, &current, apply, continue_on_error).await?;
    restore_vscode(vscode, apply).await?;
    restore_git(git, apply).await?;

    if apply {
        apply_environment(environment).await?;
    } else {
        println!(
            "  {}  would restore {} runes and {} PATH entries",
            "·".yellow().bold(),
            environment.user_variables.len().to_string().cyan().bold(),
            environment.path_entries.len().to_string().cyan().bold()
        );
    }
    Ok(())
}

async fn restore_packages(
    packages: &PackageSnapshot,
    current: &PackageSnapshot,
    apply: bool,
    continue_on_error: bool,
) -> Result<()> {
    let bar = ProgressBar::new(packages.packages.len() as u64);
    bar.set_style(ProgressStyle::with_template(
        "  {spinner:.yellow} [{elapsed_precise}] [{bar:32.yellow/blue}] {pos}/{len} {msg}",
    )?);

    for package in &packages.packages {
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

async fn apply_environment(environment: &EnvironmentSnapshot) -> Result<()> {
    let mut applied = 0usize;
    for variable in &environment.user_variables {
        if variable.name.eq_ignore_ascii_case("PATH") {
            continue;
        }
        process::checked("setx", &[&variable.name, &variable.value]).await?;
        applied += 1;
    }
    if applied > 0 {
        println!(
            "  {}  carved {} rune(s) into the environment",
            "✓".green().bold(),
            applied.to_string().cyan().bold()
        );
    }
    let path_value = environment
        .path_entries
        .iter()
        .map(|entry| entry.value.as_str())
        .collect::<Vec<_>>()
        .join(";");
    if !path_value.is_empty() {
        process::checked("setx", &["Path", &path_value]).await?;
        println!(
            "  {}  PATH bound — {} entries",
            "✓".green().bold(),
            environment.path_entries.len().to_string().cyan().bold()
        );
    }
    Ok(())
}
