use anyhow::{Context, Result};
use colored::Colorize;

use crate::cli::RestoreArgs;
use crate::core::context::AppContext;
use crate::services::{
    platform_service::PlatformService, restore_service::RestoreService, storage::SnapshotStore,
};

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

    let service = RestoreService::new(
        SnapshotStore::new(ctx.odin_dir().clone()),
        ctx.config().restore.clone(),
    );

    match &args.snapshot {
        None => {
            service.restore(args.apply, args.continue_on_error).await?;
        }
        Some(id) if service.has_local_history(id) => {
            service
                .restore_from_id(id, args.apply, args.continue_on_error)
                .await?;
        }
        Some(id) => {
            println!(
                "  {}  no local history for {} — checking the Odin Platform…",
                "·".dimmed(),
                id.cyan()
            );
            let platform = PlatformService::new(ctx.odin_dir().clone());
            let (packages, environment, vscode, git) = platform
                .fetch_snapshot(ctx.config(), id)
                .await
                .with_context(|| format!("snapshot {id} not found locally or on the platform"))?;
            service
                .restore_from_sections(
                    &packages,
                    &environment,
                    &vscode,
                    &git,
                    args.apply,
                    args.continue_on_error,
                )
                .await?;
        }
    }

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
