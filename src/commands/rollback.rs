use crate::core::context::AppContext;
use crate::services::history_service::HistoryService;
use crate::services::restore_service::RestoreService;
use crate::services::storage::SnapshotStore;
use anyhow::Result;
use colored::Colorize;

#[derive(Debug, clap::Args)]
pub struct RollbackArgs {
    /// Snapshot ID or tag to rollback to.
    pub snapshot_id: String,

    /// Apply changes without confirmation
    #[arg(long)]
    pub apply: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(ctx: AppContext, args: RollbackArgs) -> Result<()> {
    let history_service = HistoryService::new(ctx.odin_dir().clone());
    let resolved_id = history_service.resolve(&args.snapshot_id)?;
    let history = history_service.get_history()?;

    let target_snapshot = history
        .iter()
        .find(|h| h.metadata.id == resolved_id)
        .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found", resolved_id))?;

    if args.json {
        let json = serde_json::to_string_pretty(target_snapshot)?;
        println!("{}", json);
        return Ok(());
    }

    println!("{}", "Rollback Details".bold().cyan());
    println!("{}\n", "═".repeat(60));

    println!("Rolling back to snapshot: {}", resolved_id.bright_yellow());
    if let Some(tag) = &target_snapshot.metadata.tag {
        println!("Tag: {}", tag.bright_yellow());
    }
    println!("Date: {}", target_snapshot.metadata.timestamp.dimmed());
    println!("Hostname: {}", target_snapshot.metadata.hostname);
    println!(
        "Total packages: {}\n",
        target_snapshot.metadata.total_packages
    );

    if !args.apply {
        println!("{}", "Preview mode".italic().dimmed());
        println!("Use {} to apply changes", "--apply".cyan());
        println!(
            "Example: {}",
            format!("odin rollback {} --apply", args.snapshot_id).cyan()
        );
        println!();
    }

    println!(
        "{}",
        "⚠️  This will restore your environment to the selected snapshot."
            .bold()
            .yellow()
    );
    println!("This may:");
    println!("  • Install packages from the historical snapshot");
    println!("  • Restore VS Code extensions");
    println!("  • Modify Git configuration");
    println!("  • Restore environment variables and PATH entries");
    println!();

    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let restore_service = RestoreService::new(store);
    restore_service
        .restore_from_id(&resolved_id, args.apply, false)
        .await?;

    if args.apply {
        println!("\n{}", "✓ Rollback completed successfully!".green().bold());
        println!("Your environment has been restored to the selected snapshot.");
    } else {
        println!(
            "\n{}",
            "Preview complete. Re-run with --apply to actually restore."
                .italic()
                .dimmed()
        );
    }

    Ok(())
}
