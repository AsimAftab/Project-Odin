use anyhow::Result;
use colored::Colorize;
use comfy_table::{Cell, Color as TableColor};

use crate::cli::NetArgs;
use crate::core::context::AppContext;
use crate::models::doctor::Severity;
use crate::services::net_service::NetService;
use crate::ui::text_tables::{rule, styled_table};

pub async fn run(_ctx: AppContext, args: NetArgs) -> Result<()> {
    let targets = if args.target.is_empty() {
        None
    } else {
        Some(
            args.target
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        )
    };

    let report = NetService::run(targets).await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!();
    println!(
        "  {}  {}",
        "📡".bright_cyan().bold(),
        "NET — Munin watches the network".bright_white().bold()
    );
    println!("  {}", rule(60).dimmed());

    // Print proxy info if set
    let proxy = &report.proxy;
    if proxy.http_proxy.is_some() || proxy.https_proxy.is_some() || proxy.no_proxy.is_some() {
        println!("  {}", "Proxy Configuration Detected:".cyan());
        if let Some(h) = &proxy.http_proxy {
            println!("    HTTP_PROXY  = {}", h);
        }
        if let Some(h) = &proxy.https_proxy {
            println!("    HTTPS_PROXY = {}", h);
        }
        if let Some(h) = &proxy.no_proxy {
            println!("    NO_PROXY    = {}", h);
        }
        println!("  {}", rule(60).dimmed());
    }

    let mut table = styled_table(&["Target", "DNS", "HTTP", "Latency", "Status"]);

    for check in &report.checks {
        let (dns_icon, dns_color) = if check.dns_ok {
            ("✓", TableColor::Green)
        } else {
            ("✗", TableColor::Red)
        };

        let (http_icon, http_color) = if check.http_ok {
            ("✓", TableColor::Green)
        } else {
            ("✗", TableColor::Red)
        };

        let latency_str = match check.latency_ms {
            Some(ms) => format!("{} ms", ms),
            None => "—".to_string(),
        };

        let (status_label, status_color) = match check.status {
            Severity::Info => ("OK", TableColor::Green),
            Severity::Warning => ("WARN", TableColor::Yellow),
            Severity::Error => ("ERR", TableColor::Red),
        };

        table.add_row(vec![
            Cell::new(&check.target),
            Cell::new(dns_icon).fg(dns_color),
            Cell::new(http_icon).fg(http_color),
            Cell::new(&latency_str),
            Cell::new(status_label).fg(status_color),
        ]);
    }

    println!("{table}");

    let errors = report
        .checks
        .iter()
        .filter(|c| matches!(c.status, Severity::Error))
        .count();

    println!();
    if errors > 0 {
        println!(
            "  {}  {} connection(s) failed",
            "!".red().bold(),
            errors.to_string().bright_red().bold()
        );
    } else {
        println!("  {}  all connections successful", "✓".green().bold());
    }
    println!();

    Ok(())
}
