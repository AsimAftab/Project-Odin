use anyhow::Result;
use crate::core::context::AppContext;
use crate::services::history_service::HistoryService;
use chrono::{DateTime, Utc};
use colored::Colorize;

#[derive(Debug, clap::Args)]
pub struct HistoryArgs {
    /// Show detailed diffs between snapshots
    #[arg(long)]
    pub detailed: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(ctx: AppContext, args: HistoryArgs) -> Result<()> {
    let service = HistoryService::new(ctx.odin_dir().clone());
    let history = service.get_history()?;

    if history.is_empty() {
        println!("No snapshot history found. Run 'odin snapshot' first.");
        return Ok(());
    }

    if args.json {
        let json = serde_json::to_string_pretty(&history)?;
        println!("{}", json);
        return Ok(());
    }

    // Display history in human-readable format
    println!(
        "{}",
        "📚 Snapshot History".bold().cyan()
    );
    println!("{}\n", "═".repeat(60));

    for (idx, entry) in history.iter().enumerate() {
        let timestamp = format_timestamp(&entry.metadata.timestamp);
        let snap_id = entry.metadata.id.bright_yellow();

        println!(
            "{} {} ({})",
            if idx == 0 { "📍" } else { "📷" },
            timestamp,
            snap_id
        );

        if entry.changes.is_empty() {
            println!("  {} No changes from previous snapshot", "·".dimmed());
        } else {
            // Show summary
            let summary = &entry.summary;
            if summary.packages_added > 0 {
                println!(
                    "  {} {} packages added",
                    "+".green(),
                    summary.packages_added
                );
            }
            if summary.packages_removed > 0 {
                println!(
                    "  {} {} packages removed",
                    "-".red(),
                    summary.packages_removed
                );
            }
            if summary.packages_updated > 0 {
                println!(
                    "  {} {} packages updated",
                    "~".yellow(),
                    summary.packages_updated
                );
            }
            if summary.env_vars_changed > 0 {
                println!(
                    "  {} {} environment variables changed",
                    "⚙".blue(),
                    summary.env_vars_changed
                );
            }
            if summary.extensions_added > 0 {
                println!(
                    "  {} {} extensions added",
                    "+".green(),
                    summary.extensions_added
                );
            }
            if summary.extensions_removed > 0 {
                println!(
                    "  {} {} extensions removed",
                    "-".red(),
                    summary.extensions_removed
                );
            }

            // Show detailed changes if requested
            if args.detailed {
                for change in &entry.changes {
                    print_change_detailed(change);
                }
            }
        }

        println!();
    }

    // Show rollback instructions
    println!(
        "{}",
        "Rollback Instructions".underline()
    );
    if let Some(entry) = history.first() {
        println!(
            "To restore to any snapshot, use: {}",
            format!("odin rollback {}", entry.metadata.id).cyan()
        );
    }

    Ok(())
}

fn format_timestamp(iso_timestamp: &str) -> String {
    match DateTime::parse_from_rfc3339(iso_timestamp) {
        Ok(dt) => {
            let local = dt.with_timezone(&Utc);
            let now = Utc::now();
            let duration = now.signed_duration_since(local);

            if duration.num_seconds() < 60 {
                "just now".to_string()
            } else if duration.num_minutes() < 60 {
                format!("{} minutes ago", duration.num_minutes())
            } else if duration.num_hours() < 24 {
                format!("{} hours ago", duration.num_hours())
            } else if duration.num_days() < 7 {
                format!("{} days ago", duration.num_days())
            } else {
                local.format("%Y-%m-%d %H:%M:%S").to_string()
            }
        }
        Err(_) => iso_timestamp.to_string(),
    }
}

fn print_change_detailed(change: &crate::models::history::EnvironmentChange) {
    use crate::models::history::ChangeType;

    let icon = match change.change_type {
        ChangeType::Added => "+".green(),
        ChangeType::Removed => "-".red(),
        ChangeType::Updated | ChangeType::Modified => "~".yellow(),
    };

    println!(
        "    {} {} ({})",
        icon,
        change.item.bright_white(),
        change.category.dimmed()
    );

    if let Some(old_val) = &change.old_value {
        if old_val.len() < 60 {
            println!("      {} {}", "was:".dimmed(), old_val.dimmed());
        } else {
            println!(
                "      {} {}...",
                "was:".dimmed(),
                old_val.chars().take(57).collect::<String>().dimmed()
            );
        }
    }

    if let Some(new_val) = &change.new_value {
        if new_val.len() < 60 {
            println!("      {} {}", "now:".dimmed(), new_val.green());
        } else {
            println!(
                "      {} {}...",
                "now:".dimmed(),
                new_val.chars().take(57).collect::<String>().green()
            );
        }
    }
}
