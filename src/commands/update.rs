use anyhow::Result;
use colored::Colorize;

use crate::cli::UpdateArgs;
use crate::core::context::AppContext;
use crate::services::update_service::{UpdateOutcome, UpdateService};

pub async fn run(_ctx: AppContext, args: UpdateArgs) -> Result<()> {
    println!();
    println!(
        "  {}  {}",
        "ᛗ".bright_yellow().bold(),
        "UPDATE — renew Mjölnir".bright_white().bold()
    );
    println!("  {}", "─".repeat(54).dimmed());
    match UpdateService::run(args.check).await? {
        UpdateOutcome::UpToDate { current, latest } => {
            println!(
                "  {}  Mjölnir is whole — Odin runs {}",
                "✓".green().bold(),
                current.bright_yellow().bold()
            );
            if latest != current {
                println!(
                    "    {}  latest release tag: {}",
                    "·".dimmed(),
                    latest.cyan()
                );
            }
        }
        UpdateOutcome::UpdateAvailable { current, latest } => {
            println!(
                "  {}  new rune available: {} → {}",
                "!".yellow().bold(),
                current.dimmed(),
                latest.bright_yellow().bold()
            );
            println!(
                "    {}  run {} to renew the hammer",
                "→".bright_blue(),
                "odin update".cyan().bold()
            );
        }
        UpdateOutcome::UpdateStaged { current, latest } => {
            println!(
                "  {}  rune staged: {} → {}",
                "✓".green().bold(),
                current.dimmed(),
                latest.bright_yellow().bold()
            );
            println!(
                "    {}  restart your terminal to wield the renewed hammer",
                "→".bright_blue()
            );
        }
    }
    println!();
    Ok(())
}
