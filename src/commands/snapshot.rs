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
    spinner.set_style(ProgressStyle::with_template("  {spinner:.yellow} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner.set_message("Hugin & Munin survey the realm");
    let machine = SnapshotService::new(store.clone())
        .with_keep_last(ctx.config().snapshot.keep_last)
        .capture(args.include_machine_env, args.tag.clone())
        .await?;
    spinner.finish_and_clear();

    println!();
    println!(
        "  {}  {}",
        "ᛒ".bright_yellow().bold(),
        "SNAPSHOT — realm sealed in the vault".bright_white().bold()
    );
    println!("  {}", rule(60).dimmed());

    let packages = store.read_packages().await?;
    let environment = store.read_environment().await?;
    let vscode = store.read_vscode().await?;
    let git = store.read_git().await?;

    let mut table = styled_table(&["Hoard", "Count"]);
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
    println!("{table}");
    println!();

    println!(
        "  {}  rune {} sealed for realm {}",
        "✓".green().bold(),
        machine.snapshot_id.to_string().bright_yellow(),
        machine.hostname.cyan().bold()
    );
    if let Some(tag) = &args.tag {
        println!(
            "  {}  tagged as {}",
            "·".dimmed(),
            tag.bright_yellow().bold()
        );
    }
    println!(
        "  {}  vault {}",
        "·".dimmed(),
        ctx.odin_dir().display().to_string().dimmed()
    );
    println!();
    Ok(())
}
