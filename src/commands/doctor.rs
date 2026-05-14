use anyhow::Result;
use colored::Colorize;
use comfy_table::Cell;

use crate::cli::DoctorArgs;
use crate::core::context::AppContext;
use crate::models::doctor::Severity;
use crate::services::doctor_service::DoctorService;
use crate::ui::text_tables::{rule, styled_table};

pub async fn run(_ctx: AppContext, args: DoctorArgs) -> Result<()> {
    let report = DoctorService::run().await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("{}", "Doctor Report".bold().cyan());
    println!("{}\n", rule(60));
    if report.findings.is_empty() {
        println!("{} no issues found", "ok".green());
        return Ok(());
    }
    let mut table = styled_table(&["Severity", "Code", "Message", "Suggestion"]);
    for finding in &report.findings {
        let severity = match finding.severity {
            Severity::Info => "info".blue(),
            Severity::Warning => "warn".yellow(),
            Severity::Error => "error".red(),
        };
        table.add_row(vec![
            Cell::new(severity.to_string()),
            Cell::new(&finding.code),
            Cell::new(&finding.message),
            Cell::new(finding.suggestion.as_deref().unwrap_or("-")),
        ]);
    }
    println!("{table}");
    Ok(())
}
