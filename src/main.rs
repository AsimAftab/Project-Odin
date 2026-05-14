mod cli;
mod commands;
mod core;
mod integrations;
mod models;
mod services;
mod ui;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    utils::logging::init();
    let cli = Cli::parse();
    let ctx = core::context::AppContext::new(cli.odin_dir)?;

    match cli.command {
        Some(Commands::Dashboard(args)) => commands::dashboard::run(ctx, args).await,
        Some(Commands::Init(args)) => commands::init::run(ctx, args).await,
        Some(Commands::Config(args)) => commands::config::run(ctx, args).await,
        Some(Commands::Snapshot(args)) => commands::snapshot::run(ctx, args).await,
        Some(Commands::Restore(args)) => commands::restore::run(ctx, args).await,
        Some(Commands::Sync(args)) => commands::sync::run(ctx, args).await,
        Some(Commands::Update(args)) => commands::update::run(ctx, args).await,
        Some(Commands::Doctor(args)) => commands::doctor::run(ctx, args).await,
        Some(Commands::Diff(args)) => commands::diff::run(ctx, args).await,
        Some(Commands::Export(args)) => commands::export::run(ctx, args).await,
        Some(Commands::Ports(args)) => commands::ports::run(ctx, args).await,
        Some(Commands::Kill(args)) => commands::kill::run(ctx, args).await,
        Some(Commands::Ps(args)) => commands::ps::run(ctx, args).await,
        Some(Commands::History(args)) => commands::history::run(ctx, args).await,
        Some(Commands::Rollback(args)) => commands::rollback::run(ctx, args).await,
        Some(Commands::Batmode(args)) => commands::batmode::run(ctx, args).await,
        Some(Commands::Watch(args)) => commands::watch::run(ctx, args).await,
        Some(Commands::Plugin(args)) => commands::plugin::run(ctx, args).await,
        None => {
            utils::banner::print_banner();
            Ok(())
        }
    }
}
