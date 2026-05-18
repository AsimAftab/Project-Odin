use anyhow::Result;
use colored::Colorize;

use crate::cli::SyncArgs;
use crate::core::context::AppContext;
use crate::services::{storage::SnapshotStore, sync_service::SyncService};

pub async fn run(ctx: AppContext, args: SyncArgs) -> Result<()> {
    println!();
    println!(
        "  {}  {}",
        "ᛯ".bright_yellow().bold(),
        "SYNC — cross the Bifrost".bright_white().bold()
    );
    println!("  {}", "─".repeat(60).dimmed());

    let remote = args
        .remote
        .or_else(|| ctx.config().sync.remote.clone())
        .or_else(|| ctx.config().github.repository_url.clone());
    let branch = if args.branch == "main" {
        ctx.config().sync.branch.clone()
    } else {
        args.branch
    };

    SyncService::new(SnapshotStore::new(ctx.odin_dir().clone()))
        .sync(
            remote,
            args.create_private_repo,
            args.github_repo,
            args.github_token,
            &branch,
            args.message,
        )
        .await
}
