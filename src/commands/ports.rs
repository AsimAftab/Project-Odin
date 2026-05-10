use anyhow::Result;
use colored::Colorize;

use crate::cli::PortsArgs;
use crate::services::process_service::ProcessService;

pub async fn run(_ctx: crate::core::context::AppContext, args: PortsArgs) -> Result<()> {
    let ports = ProcessService::get_listening_ports().await?;

    if args.json {
        let json = serde_json::to_string_pretty(&ports)?;
        println!("{}", json);
    } else {
        if ports.is_empty() {
            println!("{}", "No listening ports found".yellow());
            return Ok(());
        }

        println!("{}", "\n=== Listening Ports ===".cyan().bold());
        println!(
            "{:<6} {:<10} {:<8} {:<30}",
            "PORT".bold(),
            "PROTOCOL".bold(),
            "PID".bold(),
            "PROCESS".bold()
        );
        println!("{}", "─".repeat(60));

        for port in ports {
            println!(
                "{:<6} {:<10} {:<8} {:<30}",
                port.port.to_string().yellow(),
                port.protocol.cyan(),
                port.pid.to_string().magenta(),
                port.process_name
            );
        }
        println!();
    }

    Ok(())
}
