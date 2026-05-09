use anyhow::Result;
use colored::Colorize;

use crate::cli::DiffArgs;
use crate::core::context::AppContext;
use crate::services::{diff_service::DiffService, storage::SnapshotStore};

pub async fn run(ctx: AppContext, args: DiffArgs) -> Result<()> {
    let report = DiffService::new(SnapshotStore::new(ctx.odin_dir().clone()))
        .diff()
        .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if report.changes.is_empty() {
        println!("{} no drift detected", "ok".green());
        return Ok(());
    }
    for change in report.changes {
        println!(
            "{} {} {}: {:?} -> {:?}",
            "diff".yellow(),
            change.category,
            change.item,
            change.before,
            change.after
        );
    }
    Ok(())
}
