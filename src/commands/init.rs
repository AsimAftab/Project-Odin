use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use crate::cli::InitArgs;
use crate::core::context::AppContext;
use crate::integrations::{git_cli, install, package_managers, process};
use crate::services::{
    config_service::ConfigService, export_service::ExportService, secret_service::SecretService,
    storage::SnapshotStore, sync_service::SyncService,
};
use crate::utils::fs;

pub async fn run(ctx: AppContext, args: InitArgs) -> Result<()> {
    println!();
    println!(
        "  {}  {}",
        "ᚷ".bright_yellow().bold(),
        "INIT — forge a fresh vault".bright_white().bold()
    );
    println!("  {}", "─".repeat(54).dimmed());
    println!("  {}  carving the vault skeleton", "·".bright_blue());
    let config_path = ConfigService::new(ctx.odin_dir().clone())
        .init(args.force)
        .await?;
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    store.ensure().await?;

    let setup_dirs = ["snapshots", "logs", "cache", "temp", "plugins"];
    for name in setup_dirs {
        fs::ensure_dir(&ctx.odin_dir().join(name)).await?;
    }

    if store.path("packages.json").exists() {
        ExportService::new(store).export_scripts(args.force).await?;
    }

    println!("  {}  validating the bindings", "·".bright_blue());
    let install_status = install::collect_status().await?;

    let mut warnings = Vec::new();
    if !process::command_exists("odin") {
        warnings.push("odin command is not available on current PATH".to_string());
    }

    if !install_status.process_has_current_directory {
        let msg = format!(
            "current executable directory is missing from PATH: {}",
            install_status.current_directory.display()
        );
        warnings.push(msg.clone());

        println!("  {}  {}", "!".yellow().bold(), msg);
        if Confirm::new()
            .with_prompt(
                "Would you like to add the current executable directory to your User PATH?",
            )
            .default(true)
            .interact()?
        {
            install::add_to_user_path(&install_status.current_directory).await?;
            println!(
                "  {}  added to User PATH. Restart your terminal to apply changes.",
                "✓".green().bold()
            );
        }
    }

    if !install_status.user_path_has_user_install_dir
        && !install_status.machine_path_has_machine_install_dir
    {
        let msg = format!(
            "persistent PATH is missing default Odin install dirs: {} or {}",
            install_status.user_install_dir.display(),
            install_status.machine_install_dir.display()
        );
        warnings.push(msg.clone());

        println!("  {}  {}", "!".yellow().bold(), msg);
        if Confirm::new()
            .with_prompt(format!(
                "Would you like to add the default Odin install directory to your User PATH? ({})",
                install_status.user_install_dir.display()
            ))
            .default(true)
            .interact()?
        {
            install::add_to_user_path(&install_status.user_install_dir).await?;
            println!(
                "  {}  added to User PATH. Restart your terminal to apply changes.",
                "✓".green().bold()
            );
        }
    }

    println!("  {}  divining dependencies", "·".bright_blue());
    let git = git_cli::executable();
    if git.is_none() {
        warnings.push("git is not installed or not discoverable".to_string());
    }
    let managers = package_managers::detect_managers().await;
    let installed_managers = managers.iter().filter(|manager| manager.installed).count();
    if installed_managers == 0 {
        warnings
            .push("no supported package managers were detected (winget/choco/scoop)".to_string());
    }

    println!("  {}  checking the Bifrost (GitHub)", "·".bright_blue());
    let config = ConfigService::new(ctx.odin_dir().clone()).load().await?;
    if let Some(repo) = config.github.repository_url {
        if let Some(token_key) = config.github.token_key {
            match SecretService::get_token(&token_key) {
                Ok(_) => {
                    let branch = config.github.branch;
                    if let Err(error) = SyncService::new(SnapshotStore::new(ctx.odin_dir().clone()))
                        .ensure_repo(Some(repo), &branch)
                        .await
                    {
                        warnings.push(format!("github sync initialization failed: {error:#}"));
                    }
                }
                Err(_) => warnings.push(
                    "github repository is configured but token is missing from credential store"
                        .to_string(),
                ),
            }
        } else {
            warnings.push("github repository is configured but no token key is set".to_string());
        }
    }

    println!();
    println!(
        "  {}  vault forged at {}",
        "✓".green().bold(),
        ctx.odin_dir().display().to_string().bright_yellow().bold()
    );
    println!(
        "    {} {}",
        "config    ".dimmed(),
        config_path.display().to_string().cyan()
    );
    println!(
        "    {} {}",
        "executable".dimmed(),
        install_status
            .current_executable
            .display()
            .to_string()
            .cyan()
    );
    println!(
        "    {} {} forge(s) detected",
        "ready     ".dimmed(),
        installed_managers.to_string().cyan().bold()
    );
    if warnings.is_empty() {
        println!("  {}  setup validation passed", "✓".green().bold());
    } else {
        for warning in warnings {
            println!("  {}  {}", "!".yellow().bold(), warning);
        }
        println!("  {}  setup completed with warnings", "!".yellow().bold());
    }
    println!();
    Ok(())
}
