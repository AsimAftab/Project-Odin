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

    println!();
    println!(
        "  {}  {}",
        "ᛞ".bright_yellow().bold(),
        "ROLLBACK — wind back the realm".bright_white().bold()
    );
    println!("  {}", "═".repeat(60).dimmed());

    println!(
        "  {}  rune     {}",
        "·".dimmed(),
        resolved_id.bright_yellow().bold()
    );
    if let Some(tag) = &target_snapshot.metadata.tag {
        println!("  {}  tag      {}", "·".dimmed(), tag.bright_cyan().bold());
    }
    println!(
        "  {}  date     {}",
        "·".dimmed(),
        target_snapshot.metadata.timestamp.cyan()
    );
    println!(
        "  {}  realm    {}",
        "·".dimmed(),
        target_snapshot.metadata.hostname.cyan()
    );
    println!(
        "  {}  packages {}",
        "·".dimmed(),
        target_snapshot
            .metadata
            .total_packages
            .to_string()
            .cyan()
            .bold()
    );
    println!();

    if !args.apply {
        println!(
            "  {}  preview only — pass {} to actually wind back",
            "·".bright_blue(),
            "--apply".cyan().bold()
        );
        println!(
            "    example: {}",
            format!("odin rollback {} --apply", args.snapshot_id)
                .cyan()
                .bold()
        );
        println!();
    }

    println!(
        "  {}  {}",
        "⚠".yellow().bold(),
        "this will rebind your realm to the selected rune"
            .yellow()
            .bold()
    );
    println!("    • install packages from the historical snapshot");
    println!("    • restore VS Code extensions");
    println!("    • modify Git configuration");
    println!("    • restore environment variables and PATH entries");
    println!();

    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let restore_service = RestoreService::new(store, ctx.config().restore.clone());
    restore_service
        .restore_from_id(&resolved_id, args.apply, false)
        .await?;

    if args.apply {
        println!();
        println!(
            "  {}  rollback complete — realm wound back to {}",
            "✓".green().bold(),
            resolved_id.bright_yellow().bold()
        );
    } else {
        println!();
        println!(
            "  {}  preview complete — re-run with {} to bind",
            "·".dimmed(),
            "--apply".cyan().bold()
        );
    }
    println!();

    Ok(())
}
