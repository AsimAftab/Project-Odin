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
    /// Human-readable command, for display and manual-list hints.
    pub display: String,
    pub program: &'static str,
    pub args: Vec<String>,
    /// Caveat printed before running.
    pub note: Option<&'static str>,
    /// Executable that must already exist for this recipe to work.
    pub requires: Option<&'static str>,
}

/// The bootstrap recipe for a manager, if we trust one. `elevated` switches
/// the scoop recipe to the officially supported admin-shell form — the plain
/// installer aborts under an elevated prompt.
pub fn recipe_for(manager: &PackageManager, elevated: bool) -> Option<BootstrapRecipe> {
    match manager {
        PackageManager::Scoop => {
            let script = if elevated {
                r#"iex "& {$(irm get.scoop.sh)} -RunAsAdmin""#
            } else {
                "iwr -useb get.scoop.sh | iex"
            };
            Some(BootstrapRecipe {
                display: format!("powershell -NoProfile -Command \"{script}\""),
                program: "powershell",
                args: vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    script.to_string(),
                ],
                note: if elevated {
                    Some("elevated shell detected — using scoop's -RunAsAdmin installer")
                } else {
                    None
                },
                requires: None,
            })
        }
        PackageManager::Chocolatey => Some(winget_recipe(
            "Chocolatey.Chocolatey",
            Some("Chocolatey needs an elevated (admin) shell"),
        )),
        PackageManager::Uv => Some(winget_recipe("astral-sh.uv", None)),
        PackageManager::Pnpm => Some(npm_recipe("pnpm")),
        PackageManager::Yarn => Some(npm_recipe("yarn")),
        PackageManager::Pipx => Some(BootstrapRecipe {
            display: "pip install --user pipx".to_string(),
            program: "pip",
            args: vec![
                "install".to_string(),
                "--user".to_string(),
                "pipx".to_string(),
            ],
            note: Some("run `pipx ensurepath` afterwards so pipx apps land on PATH"),
            requires: Some("pip"),
        }),
        // Language runtimes / core tooling: installing these is a bigger
        // decision than a restore should make silently.
        _ => None,
    }
}

fn winget_recipe(id: &str, note: Option<&'static str>) -> BootstrapRecipe {
    let args: Vec<String> = [
        "install",
        "--id",
        id,
        "-e",
        "--source",
        "winget",
        "--accept-package-agreements",
        "--accept-source-agreements",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    BootstrapRecipe {
        display: format!("winget {}", args.join(" ")),
        program: "winget",
        args,
        note,
        requires: Some("winget"),
    }
}

fn npm_recipe(package: &str) -> BootstrapRecipe {
    BootstrapRecipe {
        display: format!("npm install -g {package}"),
        program: "npm",
        args: vec!["install".to_string(), "-g".to_string(), package.to_string()],
        note: None,
        requires: Some("npm"),
    }
}

/// Manual-list hint for a missing manager: the bootstrap command if we have a
/// recipe, otherwise a runtime-install pointer.
pub fn install_hint(manager: &PackageManager) -> Option<String> {
    if let Some(recipe) = recipe_for(manager, false) {
        return Some(recipe.display);
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

/// True when the current process runs elevated (admin). `net session` exits 0
/// only for administrators; cheap and dependency-free.
async fn is_elevated() -> bool {
    process::capture("net", &["session"])
        .await
        .map(|out| out.code == 0)
        .unwrap_or(false)
}

/// Offers/performs bootstrap for every plan-missing manager. On success the
/// manager's `ManagerMissing` packages are upgraded to `WillInstall` and it is
/// removed from `missing_managers`. Declined, recipe-less, or failed managers
/// stay missing (→ manual list). Returns human-readable labels of what was
/// bootstrapped.
pub async fn bootstrap_missing(
    plan: &mut RestorePlan,
    options: &RestoreOptions,
) -> Result<Vec<String>> {
    let mut bootstrapped = Vec::new();
    if plan.missing_managers.is_empty() {
        return Ok(bootstrapped);
    }

    let can_prompt = !options.non_interactive && !options.quiet && terminal::is_interactive();
    let elevated = is_elevated().await;

    for manager in plan.missing_managers.clone() {
        let label = manager_label(&manager);
        let dependents = plan
            .packages
            .iter()
            .filter(|p| p.source == manager && p.action == PlanAction::ManagerMissing)
            .count();

        let Some(recipe) = recipe_for(&manager, elevated) else {
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
                recipe.display.dimmed()
            );
        }

        let arg_refs: Vec<&str> = recipe.args.iter().map(String::as_str).collect();
        let outcome = process::capture(recipe.program, &arg_refs).await;
        // Trust the post-install probe over the exit code: PowerShell
        // one-liners can exit 0 even when the installer aborted (seen with
        // scoop under an admin shell), and for scoop/choco the probe checks
        // well-known install paths, so it works with a stale session PATH.
        let visible = manager_visible(&manager);

        if visible {
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
            continue;
        }

        let path_independent_probe =
            matches!(manager, PackageManager::Scoop | PackageManager::Chocolatey);
        if path_independent_probe {
            // The probe checks the actual install location, so not-visible
            // means the install genuinely failed — say so honestly instead of
            // a false "restart your shell".
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
                    detail.lines().last().unwrap_or("(no output)").trim()
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
            assert!(recipe_for(&m, false).is_some(), "expected recipe for {m:?}");
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
            assert!(
                recipe_for(&m, false).is_none(),
                "no recipe expected for {m:?}"
            );
            assert!(install_hint(&m).is_some(), "hint expected for {m:?}");
        }
    }

    #[test]
    fn scoop_recipe_switches_to_run_as_admin_when_elevated() {
        let normal = recipe_for(&PackageManager::Scoop, false).unwrap();
        assert!(normal
            .args
            .last()
            .unwrap()
            .contains("iwr -useb get.scoop.sh"));
        assert!(!normal.display.contains("RunAsAdmin"));

        let admin = recipe_for(&PackageManager::Scoop, true).unwrap();
        assert!(admin.args.last().unwrap().contains("-RunAsAdmin"));
        assert_eq!(admin.program, "powershell");
        // The script is a single argv element — no shell re-quoting to break.
        assert_eq!(admin.args.len(), 3);
    }

    #[test]
    fn winget_recipes_pin_the_winget_source() {
        let choco = recipe_for(&PackageManager::Chocolatey, false).unwrap();
        assert!(choco.args.contains(&"--source".to_string()));
        assert!(choco.args.contains(&"winget".to_string()));
        assert_eq!(choco.requires, Some("winget"));
    }
}
