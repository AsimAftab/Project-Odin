use anyhow::Result;
use colored::Colorize;
use comfy_table::Cell;
use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::SnapshotArgs;
use crate::core::context::AppContext;
use crate::services::{snapshot_service::SnapshotService, storage::SnapshotStore};
use crate::ui::text_tables::{rule, styled_table};

pub async fn run(ctx: AppContext, args: SnapshotArgs) -> Result<()> {
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("{spinner:.cyan} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner.set_message("capturing developer environment");
    let machine = SnapshotService::new(store.clone())
        .capture(args.include_machine_env)
        .await?;
    spinner.finish_and_clear();

    println!("{}", "Snapshot Captured".bold().cyan());
    println!("{}\n", rule(60));

    let packages = store.read_packages().await?;
    let environment = store.read_environment().await?;
    let vscode = store.read_vscode().await?;
    let git = store.read_git().await?;

    let mut table = styled_table(&["Category", "Count"]);
    table.add_row(vec![
        Cell::new("Packages"),
        Cell::new(packages.packages.len()),
    ]);
    table.add_row(vec![
        Cell::new("User env vars"),
        Cell::new(environment.user_variables.len()),
    ]);
    table.add_row(vec![
        Cell::new("PATH entries"),
        Cell::new(environment.path_entries.len()),
    ]);
    table.add_row(vec![
        Cell::new("VS Code extensions"),
        Cell::new(vscode.extensions.len()),
    ]);
    table.add_row(vec![
        Cell::new("Git config keys"),
        Cell::new(git.entries.len()),
    ]);
    println!("{table}\n");

    println!(
        "{} snapshot {} captured for {}",
        "ok".green(),
        machine.snapshot_id.to_string().bright_yellow(),
        machine.hostname
    );
    println!("{} {}", "dir".cyan(), ctx.odin_dir().display());
    Ok(())
}
