use anyhow::Result;
use colored::Colorize;

use crate::cli::DeactivateArgs;
use crate::core::context::AppContext;
use crate::services::asgard_service;

pub async fn run(ctx: AppContext, _args: DeactivateArgs) -> Result<()> {
    println!();
    match asgard_service::deactivate(ctx.odin_dir()).await? {
        Some(name) => println!(
            "  {}  realm {} unbound",
            "✓".green().bold(),
            name.bright_yellow().bold()
        ),
        None => println!("  {}  no realm bound", "○".dimmed()),
    }
    println!(
        "  {}  env was applied per-process; spawned warriors still hold their copy until they fall",
        "·".dimmed()
    );
    println!();
    Ok(())
}
