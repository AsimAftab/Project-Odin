use anyhow::Result;
use colored::Colorize;

use crate::cli::RestoreArgs;
use crate::core::context::AppContext;
use crate::services::{restore_service::RestoreService, storage::SnapshotStore};

pub async fn run(ctx: AppContext, args: RestoreArgs) -> Result<()> {
    println!();
    let title = if args.apply {
        "RESTORE — bind realm to vault"
    } else {
        "RESTORE — preview the binding (dry-run)"
    };
    println!(
        "  {}  {}",
        "ᛞ".bright_yellow().bold(),
        title.bright_white().bold()
    );
    println!("  {}", "─".repeat(60).dimmed());

    RestoreService::new(
        SnapshotStore::new(ctx.odin_dir().clone()),
        ctx.config().restore.clone(),
    )
    .restore(args.apply, args.continue_on_error)
    .await?;

    println!();
    if args.apply {
        println!("  {}  realm bound to the vault", "✓".green().bold());
    } else {
        println!(
            "  {}  preview only — re-run with {} to bind",
            "·".dimmed(),
            "--apply".cyan().bold()
        );
    }
    println!();
    Ok(())
}
