use anyhow::Result;
use colored::Colorize;

use crate::cli::LogoutArgs;
use crate::core::context::AppContext;
use crate::services::platform_service::PlatformService;

pub async fn run(ctx: AppContext, _args: LogoutArgs) -> Result<()> {
    PlatformService::new(ctx.odin_dir().clone())
        .logout()
        .await?;

    println!();
    println!(
        "  {}  Disconnected from the Odin Platform",
        "✓".green().bold()
    );
    println!("  {}  local snapshots are untouched", "·".dimmed());
    println!();
    Ok(())
}
