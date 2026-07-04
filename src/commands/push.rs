use anyhow::{bail, Result};
use colored::Colorize;

use crate::cli::PushArgs;
use crate::core::context::AppContext;
use crate::services::platform_service::{self, PlatformService};

pub async fn run(ctx: AppContext, args: PushArgs) -> Result<()> {
    if !platform_service::is_configured(ctx.config()) {
        bail!("not connected to a platform — run `odin login` first");
    }
    let service = PlatformService::new(ctx.odin_dir().clone());

    println!();
    if args.all {
        let summary = service.upload_all_history(ctx.config()).await?;
        if summary.total == 0 {
            println!("  {}  no local snapshots to upload", "·".dimmed());
        } else {
            let failed = if summary.failed > 0 {
                format!(" ({} failed)", summary.failed).red().to_string()
            } else {
                String::new()
            };
            println!(
                "  {}  uploaded {}/{} snapshots{}",
                "✓".green().bold(),
                summary.uploaded,
                summary.total,
                failed
            );
        }
    } else {
        let id = service.upload_latest(ctx.config()).await?;
        println!(
            "  {}  snapshot {} pushed to the platform",
            "✓".green().bold(),
            id.bright_yellow()
        );
    }
    println!();
    Ok(())
}
