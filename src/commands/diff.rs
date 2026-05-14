use anyhow::Result;
use colored::Colorize;
use comfy_table::Cell;

use crate::cli::DiffArgs;
use crate::core::context::AppContext;
use crate::services::{diff_service::DiffService, storage::SnapshotStore};
use crate::ui::text_tables::{rule, styled_table};

pub async fn run(ctx: AppContext, args: DiffArgs) -> Result<()> {
    let report = DiffService::new(SnapshotStore::new(ctx.odin_dir().clone()))
        .diff()
        .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("{}", "Drift Report".bold().cyan());
    println!("{}\n", rule(60));
    if report.changes.is_empty() {
        println!("{} no drift detected", "ok".green());
        return Ok(());
    }
    let mut table = styled_table(&["Category", "Item", "Before", "After"]);
    for change in &report.changes {
        table.add_row(vec![
            Cell::new(&change.category),
            Cell::new(&change.item),
            Cell::new(format_diff_value(change.before.as_deref())),
            Cell::new(format_diff_value(change.after.as_deref())),
        ]);
    }
    println!("{table}");
    println!(
        "\n{} {} change(s) detected.",
        "diff".yellow(),
        report.changes.len()
    );
    Ok(())
}

fn format_diff_value(value: Option<&str>) -> String {
    match value {
        Some("") => "(empty)".to_string(),
        Some(s) => s.to_string(),
        None => "-".to_string(),
    }
}
