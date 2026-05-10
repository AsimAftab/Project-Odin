use crate::core::context::AppContext;
use crate::core::errors::Result;
use crate::services::history_service::HistoryService;
use crate::services::restore_service::RestoreService;
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

pub async fn handle(ctx: &AppContext, args: RollbackArgs) -> Result<()> {
    let history_service = HistoryService::new(&ctx.odin_dir);
    let history = history_service.get_history()?;

    // Find the target snapshot
    let target_snapshot = history
        .iter()
        .find(|h| h.metadata.id == args.snapshot_id)
        .ok_or_else(|| {
            crate::core::errors::AppError::NotFound(format!(
                "Snapshot '{}' not found",
                args.snapshot_id
            ))
        })?;

    if args.json {
        let json = serde_json::to_string_pretty(target_snapshot)
            .map_err(|e| crate::core::errors::AppError::JsonError(e.to_string()))?;
        println!("{}", json);
        return Ok(());
    }

    // Show what will be restored
    println!(
        "{}",
        "Rollback Details".bold().cyan()
    );
    println!("{}\n", "═".repeat(60));

    println!(
        "Rolling back to snapshot: {}",
        args.snapshot_id.bright_yellow()
    );
    println!(
        "Date: {}",
        target_snapshot.metadata.timestamp.dimmed()
    );
    println!(
        "Hostname: {}",
        target_snapshot.metadata.hostname
    );
    println!(
        "Total packages: {}\n",
        target_snapshot.metadata.total_packages
    );

    // Show what will change
    let restore_service = RestoreService::new(&ctx.odin_dir)?;
    let plan = restore_service.plan_restore_from_snapshot(&args.snapshot_id).await?;

    println!("{}", "Changes to Apply:".underline());
    println!("  {} {} packages to install", "→".green(), plan.packages.len());
    println!(
        "  {} Git config entries",
        "→".blue(),
        // Count git config entries
    );
    println!(
        "  {} VS Code extensions",
        "→".yellow(),
        // Count extensions
    );
    println!();

    if !args.apply {
        println!(
            "{}",
            "Preview mode".italic().dimmed()
        );
        println!(
            "Use {} to apply changes",
            "--apply".cyan()
        );
        println!("Example: {}", format!("odin rollback {} --apply", args.snapshot_id).cyan());
        return Ok(());
    }

    // Confirm before applying
    println!(
        "{}",
        "⚠️  This will restore your environment to the selected snapshot.".bold().yellow()
    );
    println!("This may:");
    println!("  • Uninstall packages installed since that snapshot");
    println!("  • Restore old package versions");
    println!("  • Change environment variables");
    println!("  • Modify Git configuration");
    println!();

    if !confirm_action("Apply rollback? (y/n) ") {
        println!("Rollback cancelled.");
        return Ok(());
    }

    // Apply restore
    println!("\n{}", "Applying rollback...".cyan());
    restore_service.restore_from_snapshot(&args.snapshot_id, true).await?;

    println!(
        "{}",
        "✓ Rollback completed successfully!".green().bold()
    );
    println!("Your environment has been restored to the selected snapshot.");

    Ok(())
}

fn confirm_action(prompt: &str) -> bool {
    use std::io::{self, Write};

    print!("{}", prompt);
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);

    input.trim().eq_ignore_ascii_case("y")
}
