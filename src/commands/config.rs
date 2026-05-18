use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{Confirm, Input, Password};

use crate::cli::{ConfigArgs, ConfigCommands, ConfigGithubArgs, ConfigShowArgs};
use crate::core::context::AppContext;
use crate::integrations::github::GitHubClient;
use crate::services::{
    config_service::ConfigService, secret_service::SecretService, storage::SnapshotStore,
    sync_service::SyncService,
};
use crate::utils::terminal;

pub async fn run(ctx: AppContext, args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommands::Github(args) => github(ctx, args).await,
        ConfigCommands::Show(args) => show(ctx, args).await,
    }
}

async fn github(ctx: AppContext, args: ConfigGithubArgs) -> Result<()> {
    let service = ConfigService::new(ctx.odin_dir().clone());
    let mut config = service.load().await?;

    let interactive = terminal::is_interactive() && !args.non_interactive;
    let repo = match args.repo {
        Some(repo) => repo,
        None if interactive => Input::<String>::new()
            .with_prompt("GitHub repository URL")
            .interact_text()?,
        None => anyhow::bail!("--repo is required in non-interactive mode"),
    };

    let branch = if args.branch == "main" && interactive {
        Input::<String>::new()
            .with_prompt("Branch")
            .default(config.github.branch.clone())
            .interact_text()?
    } else {
        args.branch
    };

    let token = match args.token {
        Some(token) => token,
        None if interactive => Password::new()
            .with_prompt("GitHub token")
            .allow_empty_password(false)
            .interact()?,
        None => anyhow::bail!("--token or GITHUB_TOKEN is required in non-interactive mode"),
    };

    let user = GitHubClient::new(&token)?.current_user().await?;
    let key = SecretService::token_key(&repo);
    SecretService::set_token(&key, &token)?;

    config.github.repository_url = Some(repo.clone());
    config.github.branch = branch.clone();
    config.github.token_key = Some(key);
    config.sync.remote = Some(repo.clone());
    config.sync.branch = branch.clone();
    service.save(&config).await?;

    if interactive
        && Confirm::new()
            .with_prompt("Initialize local snapshot git repository now?")
            .default(true)
            .interact()?
    {
        SyncService::new(SnapshotStore::new(ctx.odin_dir().clone()))
            .ensure_repo(Some(repo.clone()), &branch)
            .await
            .context("failed to initialize snapshot git repository")?;
    }

    let sync_now = if args.sync_now {
        true
    } else if interactive {
        Confirm::new()
            .with_prompt("Push current Odin state to GitHub now?")
            .default(false)
            .interact()?
    } else {
        false
    };

    if sync_now {
        SyncService::new(SnapshotStore::new(ctx.odin_dir().clone()))
            .sync(
                Some(repo.clone()),
                false,
                None,
                None,
                &branch,
                Some("Configure GitHub sync".to_string()),
            )
            .await
            .context("failed to push Odin state after GitHub configuration")?;
    }

    println!();
    println!(
        "  {}  Bifrost raised — connected as {}",
        "✓".green().bold(),
        user.login.bright_yellow().bold()
    );
    println!(
        "    {}  {}",
        "repo  ".dimmed(),
        config.github.repository_url.unwrap_or_default().cyan()
    );
    println!("    {}  {}", "branch".dimmed(), config.github.branch.cyan());
    if sync_now {
        println!(
            "  {}  initial backup pushed across the Bifrost",
            "✓".green().bold()
        );
    } else {
        println!(
            "  {}  run {} (or {}) to cross the Bifrost",
            "→".bright_blue(),
            "odin sync".cyan().bold(),
            "odin backup".cyan()
        );
    }
    println!();
    Ok(())
}

async fn show(ctx: AppContext, args: ConfigShowArgs) -> Result<()> {
    let config = ConfigService::new(ctx.odin_dir().clone()).load().await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }
    println!();
    println!(
        "  {}  {}",
        "ᛏ".bright_yellow().bold(),
        "CONFIG — runes carved into the vault".bright_white().bold()
    );
    println!("  {}", "─".repeat(54).dimmed());
    println!("{}", serde_yaml::to_string(&config)?);
    Ok(())
}
