use anyhow::Result;
use colored::Colorize;

use crate::cli::DeactivateArgs;
use crate::core::context::AppContext;
use crate::services::asgard_service;

pub async fn run(ctx: AppContext, _args: DeactivateArgs) -> Result<()> {
    match asgard_service::deactivate(ctx.odin_dir()).await? {
        Some(name) => println!("{} cleared active profile {}", "ok".green(), name.cyan()),
        None => println!("{} no active profile", "·".dimmed()),
    }
    println!(
        "{} env was applied per-process; spawned apps still hold their copy until they exit",
        "note".dimmed()
    );
    Ok(())
}
