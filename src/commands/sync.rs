use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::SyncArgs;
use crate::core::context::AppContext;
use crate::services::{
    platform_service::{self, PlatformService},
    snapshot_service::SnapshotService,
    storage::SnapshotStore,
    sync_service::SyncService,
};

/// Unified sync: capture a fresh snapshot, upload it to the Odin Platform when
/// connected, and push to GitHub when a remote is configured. Each remote that
/// isn't set up is skipped with a hint instead of a hard error.
pub async fn run(ctx: AppContext, args: SyncArgs) -> Result<()> {
    println!();
    println!(
        "  {}  {}",
        "ᛯ".bright_yellow().bold(),
        "SYNC — one command, every realm".bright_white().bold()
    );
    println!("  {}", "─".repeat(60).dimmed());

    // 1. Capture a fresh snapshot so both remotes send current state.
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("  {spinner:.yellow} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner.set_message("capturing the realm");
    let machine = SnapshotService::new(store.clone())
        .with_keep_last(ctx.config().snapshot.keep_last)
        .capture(false, args.message.clone())
        .await?;
    spinner.finish_and_clear();
    println!(
        "  {}  snapshot {} captured",
        "✓".green().bold(),
        machine
            .snapshot_id
            .to_string()
            .chars()
            .take(8)
            .collect::<String>()
            .bright_yellow()
    );

    // 2. Odin Platform (non-fatal).
    let platform_result: Option<bool> = if platform_service::is_configured(ctx.config()) {
        match PlatformService::new(ctx.odin_dir().clone())
            .upload_latest(ctx.config())
            .await
        {
            Ok(_) => {
                println!("  {}  platform: uploaded", "✓".green().bold());
                Some(true)
            }
            Err(e) => {
                println!(
                    "  {}  platform: upload failed — {}",
                    "⚠".yellow().bold(),
                    e.to_string().red()
                );
                Some(false)
            }
        }
    } else {
        println!(
            "  {}  platform: not connected ({})",
            "·".dimmed(),
            "odin login".cyan()
        );
        None
    };

    // 3. GitHub (only when a remote is configured or requested).
    let remote = args
        .remote
        .or_else(|| ctx.config().sync.remote.clone())
        .or_else(|| ctx.config().github.repository_url.clone());
    let branch = if args.branch == "main" {
        ctx.config().sync.branch.clone()
    } else {
        args.branch
    };

    let github_requested = remote.is_some() || args.create_private_repo;
    let github_result: Option<bool> = if github_requested {
        match SyncService::new(store)
            .sync(
                remote,
                args.create_private_repo,
                args.github_repo,
                args.github_token,
                &branch,
                args.message,
            )
            .await
        {
            Ok(()) => Some(true),
            Err(e) => {
                println!(
                    "  {}  github: push failed — {}",
                    "⚠".yellow().bold(),
                    e.to_string().red()
                );
                Some(false)
            }
        }
    } else {
        println!(
            "  {}  github: not configured ({})",
            "·".dimmed(),
            "odin config github".cyan()
        );
        None
    };

    // 4. Summary.
    println!();
    println!(
        "  {}  synced → platform {} · github {}",
        "ᛯ".bright_yellow().bold(),
        outcome(platform_result),
        outcome(github_result)
    );
    println!();
    Ok(())
}

fn outcome(result: Option<bool>) -> String {
    match result {
        Some(true) => "✓".green().bold().to_string(),
        Some(false) => "✗".red().bold().to_string(),
        None => "skipped".dimmed().to_string(),
    }
}
