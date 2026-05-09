use anyhow::Result;
use colored::Colorize;

use crate::cli::DoctorArgs;
use crate::core::context::AppContext;
use crate::models::doctor::Severity;
use crate::services::doctor_service::DoctorService;

pub async fn run(_ctx: AppContext, args: DoctorArgs) -> Result<()> {
    let report = DoctorService::run().await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if report.findings.is_empty() {
        println!("{} no issues found", "ok".green());
        return Ok(());
    }
    for finding in report.findings {
        let label = match finding.severity {
            Severity::Info => "info".blue(),
            Severity::Warning => "warn".yellow(),
            Severity::Error => "error".red(),
        };
        println!("{} [{}] {}", label, finding.code, finding.message);
        if let Some(suggestion) = finding.suggestion {
            println!("  suggestion: {suggestion}");
        }
    }
    Ok(())
}
