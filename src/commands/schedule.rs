use anyhow::Result;
use colored::Colorize;

use crate::cli::{ScheduleArgs, ScheduleCommands, ScheduleEnableArgs, ScheduleInterval};
use crate::core::context::AppContext;
use crate::services::schedule_service;

pub async fn run(_ctx: AppContext, args: ScheduleArgs) -> Result<()> {
    match args.command {
        ScheduleCommands::Enable(enable_args) => enable(enable_args).await,
        ScheduleCommands::Disable(_) => disable().await,
        ScheduleCommands::Status(_) => status().await,
    }
}

async fn enable(args: ScheduleEnableArgs) -> Result<()> {
    schedule_service::enable(args.interval, &args.time, args.push).await?;

    let cadence = match args.interval {
        ScheduleInterval::Daily => format!("daily at {}", args.time),
        ScheduleInterval::Hourly => "every hour".to_string(),
    };
    println!();
    println!(
        "  {}  Scheduled `odin snapshot{}` — {}",
        "✓".green().bold(),
        if args.push { " --push" } else { "" },
        cadence.cyan()
    );
    println!(
        "  {}  runs as the current user; disable with {}",
        "·".dimmed(),
        "odin schedule disable".cyan()
    );
    println!();
    Ok(())
}

async fn disable() -> Result<()> {
    schedule_service::disable().await?;
    println!();
    println!(
        "  {}  Removed the scheduled snapshot task",
        "✓".green().bold()
    );
    println!();
    Ok(())
}

async fn status() -> Result<()> {
    let exists = schedule_service::status().await?;
    println!();
    if exists {
        println!(
            "  {}  A scheduled snapshot task is registered ({})",
            "✓".green().bold(),
            schedule_service::TASK_NAME.cyan()
        );
    } else {
        println!(
            "  {}  No scheduled snapshot task — enable with {}",
            "·".dimmed(),
            "odin schedule enable".cyan()
        );
    }
    println!();
    Ok(())
}
