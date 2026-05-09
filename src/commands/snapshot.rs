use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::SnapshotArgs;
use crate::core::context::AppContext;
use crate::services::{snapshot_service::SnapshotService, storage::SnapshotStore};

pub async fn run(ctx: AppContext, args: SnapshotArgs) -> Result<()> {
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("{spinner:.cyan} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner.set_message("capturing developer environment");
    let machine = SnapshotService::new(store)
        .capture(args.include_machine_env)
        .await?;
    spinner.finish_and_clear();
    println!(
        "{} snapshot {} captured for {}",
        "ok".green(),
        machine.snapshot_id,
        machine.hostname
    );
    println!("{} {}", "dir".cyan(), ctx.odin_dir().display());
    Ok(())
}
