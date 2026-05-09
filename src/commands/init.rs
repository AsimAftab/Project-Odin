use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::InitArgs;
use crate::core::context::AppContext;
use crate::services::{
    config_service::ConfigService, export_service::ExportService, storage::SnapshotStore,
};

pub async fn run(ctx: AppContext, args: InitArgs) -> Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("{spinner:.cyan} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner.set_message("initializing Odin workspace");

    let config_path = ConfigService::new(ctx.odin_dir().clone())
        .init(args.force)
        .await?;
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    store.ensure().await?;

    if store.path("packages.json").exists() {
        ExportService::new(store).export_scripts(args.force).await?;
    }

    spinner.finish_and_clear();
    println!("{} initialized {}", "ok".green(), ctx.odin_dir().display());
    println!("{} {}", "config".cyan(), config_path.display());
    Ok(())
}
