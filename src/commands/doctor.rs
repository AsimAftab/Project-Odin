use anyhow::Result;
use colored::Colorize;
use comfy_table::{Cell, Color as TableColor};

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
    println!();
    println!(
        "  {}  {}",
        "ᛟ".bright_yellow().bold(),
        "DOCTOR — Eir tends the realm".bright_white().bold()
    );
    println!("  {}", rule(60).dimmed());
    if report.findings.is_empty() {
        println!(
            "  {}  the realm is whole — no wounds found",
            "✓".green().bold()
        );
        println!();
        return Ok(());
    }
    let mut table = styled_table(&["Severity", "Code", "Message", "Suggestion"]);
    for finding in &report.findings {
        let (label, color) = match finding.severity {
            Severity::Info => ("info", TableColor::Blue),
            Severity::Warning => ("warn", TableColor::Yellow),
            Severity::Error => ("wound", TableColor::Red),
        };
        table.add_row(vec![
            Cell::new(label).fg(color),
            Cell::new(&finding.code),
            Cell::new(&finding.message),
            Cell::new(finding.suggestion.as_deref().unwrap_or("—")),
        ]);
    }
    println!("{table}");
    let warnings = report
        .findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Warning | Severity::Error))
        .count();
    println!();
    if warnings > 0 {
        println!(
            "  {}  {} wound(s) need tending",
            "!".yellow().bold(),
            warnings.to_string().bright_yellow().bold()
        );
    } else {
        println!(
            "  {}  only informational notes — realm is sound",
            "·".dimmed()
        );
    }
    println!();
    Ok(())
}
