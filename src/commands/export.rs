use anyhow::Result;
use colored::Colorize;

use crate::cli::ExportArgs;
use crate::core::context::AppContext;
use crate::services::{export_service::ExportService, storage::SnapshotStore};

pub async fn run(ctx: AppContext, args: ExportArgs) -> Result<()> {
    ExportService::new(SnapshotStore::new(ctx.odin_dir().clone()))
        .export_scripts(args.force)
        .await?;
    println!(
        "{} scripts exported to {}",
        "ok".green(),
        ctx.odin_dir().display()
    );
    Ok(())
}
