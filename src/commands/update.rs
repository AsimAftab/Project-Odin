use anyhow::Result;
use colored::Colorize;

use crate::cli::UpdateArgs;
use crate::core::context::AppContext;
use crate::services::update_service::{UpdateOutcome, UpdateService};

pub async fn run(_ctx: AppContext, args: UpdateArgs) -> Result<()> {
    match UpdateService::run(args.check).await? {
        UpdateOutcome::UpToDate { current, latest } => {
            println!("{} Odin is up to date ({}).", "ok".green(), current);
            if latest != current {
                println!("{} latest release tag: {}", "info".cyan(), latest);
            }
        }
        UpdateOutcome::UpdateAvailable { current, latest } => {
            println!(
                "{} update available: {} -> {}",
                "info".yellow(),
                current,
                latest
            );
        }
        UpdateOutcome::UpdateStaged { current, latest } => {
            println!("{} update staged: {} -> {}", "ok".green(), current, latest);
            println!(
                "{} restart your terminal to use the updated binary once this process exits",
                "next".cyan()
            );
        }
    }
    Ok(())
}
