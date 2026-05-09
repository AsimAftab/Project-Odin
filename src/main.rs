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
        Commands::Dashboard(args) => commands::dashboard::run(ctx, args).await,
        Commands::Init(args) => commands::init::run(ctx, args).await,
        Commands::Config(args) => commands::config::run(ctx, args).await,
        Commands::Snapshot(args) => commands::snapshot::run(ctx, args).await,
        Commands::Restore(args) => commands::restore::run(ctx, args).await,
        Commands::Sync(args) => commands::sync::run(ctx, args).await,
        Commands::Update(args) => commands::update::run(ctx, args).await,
        Commands::Doctor(args) => commands::doctor::run(ctx, args).await,
        Commands::Diff(args) => commands::diff::run(ctx, args).await,
        Commands::Export(args) => commands::export::run(ctx, args).await,
    }
}
