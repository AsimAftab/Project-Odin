use anyhow::Result;
use colored::Colorize;

use crate::cli::PortsArgs;
use crate::services::process_service::ProcessService;

pub async fn run(_ctx: crate::core::context::AppContext, args: PortsArgs) -> Result<()> {
    let ports = ProcessService::get_listening_ports().await?;

    if args.json {
        let json = serde_json::to_string_pretty(&ports)?;
        println!("{}", json);
        return Ok(());
    }

    println!();
    println!(
        "  {}  {}",
        "ᛇ".bright_yellow().bold(),
        "BINDINGS — listening ports".bright_white().bold()
    );
    println!("  {}", "─".repeat(60).dimmed());

    if ports.is_empty() {
        println!("  {}  no realm holds an open binding", "·".dimmed());
        println!();
        return Ok(());
    }

    println!(
        "  {:<6} {:<10} {:<8} {}",
        "PORT".bright_yellow().bold(),
        "PROTO".bright_yellow().bold(),
        "PID".bright_yellow().bold(),
        "PROCESS".bright_yellow().bold()
    );
    println!("  {}", "·".repeat(60).dimmed());

    for port in ports {
        println!(
            "  {:<6} {:<10} {:<8} {}",
            port.port.to_string().bright_blue().bold(),
            port.protocol.dimmed(),
            port.pid.to_string().magenta(),
            port.process_name
        );
    }
    println!();
    println!(
        "  {}  free a binding with {}",
        "→".dimmed(),
        "odin freeport <PORT|PID> --force".cyan()
    );
    println!();

    Ok(())
}
