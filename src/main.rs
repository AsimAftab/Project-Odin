mod cli;
mod commands;
mod core;
mod integrations;
mod models;
mod services;
mod ui;
mod utils;

mod asgard;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    utils::logging::init();
    let cli = Cli::parse();
    let ctx = core::context::AppContext::new(cli.odin_dir)?;

    match cli.command {
        Some(Commands::AllEye(args)) => commands::all_eye::run(ctx, args).await,
        Some(Commands::Init(args)) => commands::init::run(ctx, args).await,
        Some(Commands::Config(args)) => commands::config::run(ctx, args).await,
        Some(Commands::Login(args)) => commands::login::run(ctx, args).await,
        Some(Commands::Logout(args)) => commands::logout::run(ctx, args).await,
        Some(Commands::Push(args)) => commands::push::run(ctx, args).await,
        Some(Commands::Snapshot(args)) => commands::snapshot::run(ctx, args).await,
        Some(Commands::Restore(args)) => commands::restore::run(ctx, args).await,
        Some(Commands::Sync(args)) => commands::sync::run(ctx, args).await,
        Some(Commands::Update(args)) => commands::update::run(ctx, args).await,
        Some(Commands::Doctor(args)) => commands::doctor::run(ctx, args).await,
        Some(Commands::Diff(args)) => commands::diff::run(ctx, args).await,
        Some(Commands::Export(args)) => commands::export::run(ctx, args).await,
        Some(Commands::Ports(args)) => commands::ports::run(ctx, args).await,
        Some(Commands::Freeport(args)) => commands::freeport::run(ctx, args).await,
        Some(Commands::Ps(args)) => commands::ps::run(ctx, args).await,
        Some(Commands::History(args)) => commands::history::run(ctx, args).await,
        Some(Commands::Rollback(args)) => commands::rollback::run(ctx, args).await,
        Some(Commands::Batmode(args)) => commands::batmode::run(ctx, args).await,
        Some(Commands::Watch(args)) => commands::watch::run(ctx, args).await,
        Some(Commands::Plugin(args)) => commands::plugin::run(ctx, args).await,
        Some(Commands::Archive(args)) => commands::archive::run(ctx, args).await,
        Some(Commands::Activate(args)) => commands::activate::run(ctx, args).await,
        Some(Commands::Asgard(args)) => {
            let activate_args = cli::ActivateArgs {
                name: Some(crate::asgard::profile::RESERVED_NAME.to_string()),
                non_interactive: args.non_interactive,
                json: args.json,
            };
            commands::activate::run(ctx, activate_args).await
        }
        Some(Commands::Deactivate(args)) => commands::deactivate::run(ctx, args).await,
        Some(Commands::Profile(args)) => commands::profile::run(ctx, args).await,
        Some(Commands::Current(args)) => commands::current::run(ctx, args).await,
        Some(Commands::Net(args)) => commands::net::run(ctx, args).await,
        None => {
            let active = crate::asgard::store::AsgardStore::new(ctx.odin_dir())
                .load_state()
                .await
                .ok()
                .and_then(|s| s.active_profile);
            utils::banner::print_banner(active.as_deref());
            Ok(())
        }
    }
}
