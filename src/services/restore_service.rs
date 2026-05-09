use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::integrations::{git_cli, process, vscode as vscode_integration};
use crate::models::package::InstalledPackage;
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
        let current = crate::integrations::package_managers::list_packages().await?;
        let bar = ProgressBar::new(packages.packages.len() as u64);
        bar.set_style(ProgressStyle::with_template(
            "{spinner:.cyan} [{elapsed_precise}] [{bar:32.cyan/blue}] {pos}/{len} {msg}",
        )?);

        for package in &packages.packages {
            bar.set_message(package.id.clone());
            if installed(package, &current.packages) {
                println!("{} {}", "skip".green(), package.id);
                bar.inc(1);
                continue;
            }
            let Some(command) = &package.install_command else {
                println!(
                    "{} {} has no install command",
                    "manual".yellow(),
                    package.id
                );
                bar.inc(1);
                continue;
            };
            println!(
                "{} {}",
                if apply {
                    "run".cyan()
                } else {
                    "dry-run".yellow()
                },
                command
            );
            if apply {
                let (program, args) = split_command(command);
                let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
                let result = process::checked(&program, &arg_refs).await;
                if let Err(error) = result {
                    if continue_on_error {
                        eprintln!("{} {error:#}", "failed".red());
                    } else {
                        return Err(error);
                    }
                }
            }
            bar.inc(1);
        }
        bar.finish_and_clear();

        for extension in &vscode.extensions {
            let command = format!("code --install-extension {}", extension.identifier);
            println!(
                "{} {}",
                if apply {
                    "run".cyan()
                } else {
                    "dry-run".yellow()
                },
                command
            );
            if apply {
                if let Some(code) = vscode_integration::executable() {
                    process::checked(&code, &["--install-extension", &extension.identifier])
                        .await?;
                }
            }
        }

        for entry in &git.entries {
            println!(
                "{} git config --global {} <value>",
                if apply {
                    "run".cyan()
                } else {
                    "dry-run".yellow()
                },
                entry.key
            );
            if apply {
                if let Some(git) = git_cli::executable() {
                    process::checked(&git, &["config", "--global", &entry.key, &entry.value])
                        .await?;
                }
            }
        }

        if apply {
            apply_environment(&environment).await?;
        } else {
            println!(
                "{} would restore {} environment variables and {} PATH entries",
                "dry-run".yellow(),
                environment.user_variables.len(),
                environment.path_entries.len()
            );
        }
        Ok(())
    }
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
    environment: &crate::models::environment::EnvironmentSnapshot,
) -> Result<()> {
    for variable in &environment.user_variables {
        if variable.name.eq_ignore_ascii_case("PATH") {
            continue;
        }
        process::checked("setx", &[&variable.name, &variable.value]).await?;
    }
    let path_value = environment
        .path_entries
        .iter()
        .map(|entry| entry.value.as_str())
        .collect::<Vec<_>>()
        .join(";");
    if !path_value.is_empty() {
        process::checked("setx", &["Path", &path_value]).await?;
    }
    Ok(())
}
