use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{Confirm, Input, Password};

use crate::cli::{ConfigArgs, ConfigCommands, ConfigGithubArgs, ConfigShowArgs};
use crate::core::context::AppContext;
use crate::integrations::github::GitHubClient;
use crate::services::{config_service::ConfigService, secret_service::SecretService};
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
        crate::services::sync_service::SyncService::new(
            crate::services::storage::SnapshotStore::new(ctx.odin_dir().clone()),
        )
        .ensure_repo(Some(repo), &branch)
        .await
        .context("failed to initialize snapshot git repository")?;
    }

    println!("{} GitHub connected as {}", "ok".green(), user.login);
    println!(
        "{} {}",
        "repo".cyan(),
        config.github.repository_url.unwrap_or_default()
    );
    println!("{} {}", "branch".cyan(), config.github.branch);
    Ok(())
}

async fn show(ctx: AppContext, args: ConfigShowArgs) -> Result<()> {
    let config = ConfigService::new(ctx.odin_dir().clone()).load().await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        println!("{}", serde_yaml::to_string(&config)?);
    }
    Ok(())
}
