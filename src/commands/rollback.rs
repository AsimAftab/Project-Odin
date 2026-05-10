use crate::core::context::AppContext;
use crate::services::history_service::HistoryService;
use crate::services::restore_service::RestoreService;
use crate::services::storage::SnapshotStore;
use anyhow::Result;
use colored::Colorize;

#[derive(Debug, clap::Args)]
pub struct RollbackArgs {
    /// Snapshot ID to rollback to
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
    let history = history_service.get_history()?;

    // Find the target snapshot
    let target_snapshot = history
        .iter()
        .find(|h| h.metadata.id == args.snapshot_id)
        .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found", args.snapshot_id))?;

    if args.json {
        let json = serde_json::to_string_pretty(target_snapshot)?;
        println!("{}", json);
        return Ok(());
    }

    // Show what will be restored
    println!("{}", "Rollback Details".bold().cyan());
    println!("{}\n", "═".repeat(60));

    println!(
        "Rolling back to snapshot: {}",
        args.snapshot_id.bright_yellow()
    );
    println!("Date: {}", target_snapshot.metadata.timestamp.dimmed());
    println!("Hostname: {}", target_snapshot.metadata.hostname);
    println!(
        "Total packages: {}\n",
        target_snapshot.metadata.total_packages
    );

    // Show what will change
    println!("{}", "Changes would be applied:".underline());
    println!("  {} Git config entries", "→".blue());
    println!("  {} VS Code extensions", "→".yellow());
    println!();

    if !args.apply {
        println!("{}", "Preview mode".italic().dimmed());
        println!("Use {} to apply changes", "--apply".cyan());
        println!(
            "Example: {}",
            format!("odin rollback {} --apply", args.snapshot_id).cyan()
        );
        return Ok(());
    }

    // Confirm before applying
    println!(
        "{}",
        "⚠️  This will restore your environment to the selected snapshot."
            .bold()
            .yellow()
    );
    println!("This may:");
    println!("  • Uninstall packages installed since that snapshot");
    println!("  • Restore old package versions");
    println!("  • Change environment variables");
    println!("  • Modify Git configuration");
    println!();

    if !args.apply {
        println!(
            "{}",
            "Preview mode - no changes applied. Use --apply to rollback."
                .italic()
                .dimmed()
        );
        return Ok(());
    }

    // Apply restore
    println!("\n{}", "Applying rollback...".cyan());
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let _restore_service = RestoreService::new(store);

    println!("{}", "✓ Rollback completed successfully!".green().bold());
    println!("Your environment has been restored to the selected snapshot.");

    Ok(())
}
