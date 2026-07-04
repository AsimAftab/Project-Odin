//! Bootstrapping missing package managers during restore.
//!
//! When a restore plan needs a manager that isn't installed (e.g. scoop on a
//! fresh VM), we can install the manager itself first. Only managers with a
//! safe, well-known one-liner get a recipe; language runtimes (Node, Go,
//! Python, Rust, .NET) are out of scope — their packages go to the manual
//! list with a runtime-install hint instead.

use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use crate::integrations::process;
use crate::models::package::PackageManager;
use crate::models::restore::{manager_label, PlanAction, RestorePlan};
use crate::services::restore_service::RestoreOptions;
use crate::utils::terminal;

pub struct BootstrapRecipe {
    pub command: &'static str,
    /// Caveat printed before running.
    pub note: Option<&'static str>,
    /// Executable that must already exist for this recipe to work.
    pub requires: Option<&'static str>,
}

/// The bootstrap one-liner for a manager, if we trust one.
pub fn recipe_for(manager: &PackageManager) -> Option<BootstrapRecipe> {
    match manager {
        PackageManager::Scoop => Some(BootstrapRecipe {
            command: r#"powershell -NoProfile -Command "iwr -useb get.scoop.sh | iex""#,
            note: Some(
                "scoop refuses elevated shells — if this fails under an admin prompt, \
                 re-run odin in a regular shell",
            ),
            requires: None,
        }),
        PackageManager::Chocolatey => Some(BootstrapRecipe {
            command: "winget install --id Chocolatey.Chocolatey -e --source winget --accept-package-agreements --accept-source-agreements",
            note: Some("Chocolatey needs an elevated (admin) shell"),
            requires: Some("winget"),
        }),
        PackageManager::Uv => Some(BootstrapRecipe {
            command: "winget install --id astral-sh.uv -e --source winget --accept-package-agreements --accept-source-agreements",
            note: None,
            requires: Some("winget"),
        }),
        PackageManager::Pnpm => Some(BootstrapRecipe {
            command: "npm install -g pnpm",
            note: None,
            requires: Some("npm"),
        }),
        PackageManager::Yarn => Some(BootstrapRecipe {
            command: "npm install -g yarn",
            note: None,
            requires: Some("npm"),
        }),
        PackageManager::Pipx => Some(BootstrapRecipe {
            command: "pip install --user pipx",
            note: Some("run `pipx ensurepath` afterwards so pipx apps land on PATH"),
            requires: Some("pip"),
        }),
        // Language runtimes / core tooling: installing these is a bigger
        // decision than a restore should make silently.
        _ => None,
    }
}

/// Manual-list hint for a missing manager: the bootstrap command if we have a
/// recipe, otherwise a runtime-install pointer.
pub fn install_hint(manager: &PackageManager) -> Option<String> {
    if let Some(recipe) = recipe_for(manager) {
        return Some(recipe.command.to_string());
    }
    let hint = match manager {
        PackageManager::Npm => "install Node.js: winget install OpenJS.NodeJS.LTS",
        PackageManager::Pip => "install Python: winget install Python.Python.3.12",
        PackageManager::Cargo => "install Rust: winget install Rustlang.Rustup",
        PackageManager::Go => "install Go: winget install GoLang.Go",
        PackageManager::DotnetTool => "install the .NET SDK: winget install Microsoft.DotNet.SDK.8",
        PackageManager::Winget => "install App Installer from the Microsoft Store",
        _ => return None,
    };
    Some(hint.to_string())
}

/// Offers/performs bootstrap for every plan-missing manager. On success the
/// manager's `ManagerMissing` packages are upgraded to `WillInstall` and it is
/// removed from `missing_managers`. Declined, recipe-less, or failed managers
/// stay missing (→ manual list). Returns human-readable labels of what was
/// bootstrapped (suffixed when a shell restart is needed).
pub async fn bootstrap_missing(
    plan: &mut RestorePlan,
    options: &RestoreOptions,
) -> Result<Vec<String>> {
    let mut bootstrapped = Vec::new();
    if plan.missing_managers.is_empty() {
        return Ok(bootstrapped);
    }

    let can_prompt = !options.non_interactive && !options.quiet && terminal::is_interactive();

    for manager in plan.missing_managers.clone() {
        let label = manager_label(&manager);
        let dependents = plan
            .packages
            .iter()
            .filter(|p| p.source == manager && p.action == PlanAction::ManagerMissing)
            .count();

        let Some(recipe) = recipe_for(&manager) else {
            if !options.quiet {
                println!(
                    "  {}  {} is not installed ({} package(s) need it) — {}",
                    "!".yellow().bold(),
                    label.cyan(),
                    dependents,
                    install_hint(&manager).unwrap_or_else(|| "install it manually".into())
                );
            }
            continue;
        };

        if let Some(required) = recipe.requires {
            if !process::command_exists(required) {
                if !options.quiet {
                    println!(
                        "  {}  can't bootstrap {} — it needs {} which is also missing",
                        "!".yellow().bold(),
                        label.cyan(),
                        required.cyan()
                    );
                }
                continue;
            }
        }

        let go = if options.bootstrap_managers {
            true
        } else if can_prompt {
            Confirm::new()
                .with_prompt(format!(
                    "{label} is not installed but {dependents} package(s) need it. Install {label} now?"
                ))
                .default(false)
                .interact()?
        } else {
            if !options.quiet {
                println!(
                    "  {}  {} missing ({} package(s)) — pass {} or install manually",
                    "·".dimmed(),
                    label.cyan(),
                    dependents,
                    "--bootstrap-managers".cyan()
                );
            }
            false
        };
        if !go {
            continue;
        }

        if let Some(note) = recipe.note {
            if !options.quiet {
                println!("  {}  {}", "·".dimmed(), note.dimmed());
            }
        }
        if !options.quiet {
            println!(
                "  {}  {}",
                "→".bright_blue().bold(),
                recipe.command.dimmed()
            );
        }

        let (program, args) = split(recipe.command);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let outcome = process::capture(&program, &arg_refs).await;
        let succeeded = matches!(&outcome, Ok(out) if out.code == 0);

        if !succeeded {
            if !options.quiet {
                let detail = match &outcome {
                    Ok(out) if !out.stderr.is_empty() => out.stderr.clone(),
                    Ok(out) => out.stdout.clone(),
                    Err(e) => format!("{e:#}"),
                };
                eprintln!(
                    "  {}  bootstrapping {} failed: {}",
                    "✗".red().bold(),
                    label.cyan(),
                    detail.lines().last().unwrap_or("").trim()
                );
            }
            continue;
        }

        // The new shims may not be on this process's PATH yet — re-probe via
        // the candidate-path-aware detectors where we have them.
        if manager_visible(&manager) {
            for p in plan.packages.iter_mut() {
                if p.source == manager && p.action == PlanAction::ManagerMissing {
                    p.action = PlanAction::WillInstall;
                }
            }
            plan.missing_managers.retain(|m| m != &manager);
            bootstrapped.push(label.to_string());
            if !options.quiet {
                println!(
                    "  {}  {} installed — continuing",
                    "✓".green().bold(),
                    label.cyan()
                );
            }
        } else {
            bootstrapped.push(format!("{label} (restart shell)"));
            if !options.quiet {
                println!(
                    "  {}  {} installed, but not visible in this session's PATH — its packages \
                     stay on the manual list; restart your shell and re-run odin restore --apply",
                    "!".yellow().bold(),
                    label.cyan()
                );
            }
        }
    }

    Ok(bootstrapped)
}

/// Post-bootstrap visibility check. choco/scoop have candidate-path fallbacks
/// that work even when the current process PATH is stale; the rest rely on
/// PATH (`npm install -g` bins usually land in an already-on-PATH prefix).
fn manager_visible(manager: &PackageManager) -> bool {
    match manager {
        PackageManager::Chocolatey => {
            crate::integrations::package_managers::choco_executable().is_some()
        }
        PackageManager::Scoop => {
            crate::integrations::package_managers::scoop_executable().is_some()
        }
        other => process::command_exists(manager_label(other)),
    }
}

fn split(command: &str) -> (String, Vec<String>) {
    // Recipes with embedded quoted segments (the scoop one-liner) must keep
    // the quoted part as a single argument, so split on quotes first.
    if let Some(idx) = command.find('"') {
        let head = command[..idx].trim();
        let quoted = command[idx..].trim_matches('"').to_string();
        let mut parts = head.split_whitespace();
        let program = parts.next().unwrap_or_default().to_string();
        let mut args: Vec<String> = parts.map(ToOwned::to_owned).collect();
        args.push(quoted);
        (program, args)
    } else {
        let mut parts = command.split_whitespace();
        let program = parts.next().unwrap_or_default().to_string();
        (program, parts.map(ToOwned::to_owned).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes_exist_for_bootstrappable_managers() {
        for m in [
            PackageManager::Scoop,
            PackageManager::Chocolatey,
            PackageManager::Uv,
            PackageManager::Pnpm,
            PackageManager::Yarn,
            PackageManager::Pipx,
        ] {
            assert!(recipe_for(&m).is_some(), "expected recipe for {m:?}");
        }
    }

    #[test]
    fn runtimes_have_hints_but_no_recipes() {
        for m in [
            PackageManager::Npm,
            PackageManager::Pip,
            PackageManager::Cargo,
            PackageManager::Go,
            PackageManager::DotnetTool,
            PackageManager::Winget,
        ] {
            assert!(recipe_for(&m).is_none(), "no recipe expected for {m:?}");
            assert!(install_hint(&m).is_some(), "hint expected for {m:?}");
        }
    }

    #[test]
    fn split_keeps_quoted_segment_intact() {
        let (program, args) =
            split(r#"powershell -NoProfile -Command "iwr -useb get.scoop.sh | iex""#);
        assert_eq!(program, "powershell");
        assert_eq!(args.last().unwrap(), "iwr -useb get.scoop.sh | iex");
        assert!(args.contains(&"-Command".to_string()));
    }
}
