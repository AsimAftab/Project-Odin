use anyhow::Result;
use colored::Colorize;

use crate::cli::KillArgs;
use crate::services::process_service::ProcessService;

pub async fn run(_ctx: crate::core::context::AppContext, args: KillArgs) -> Result<()> {
    let target = &args.target;

    // Smart detection: determine if input is port or PID
    let is_port = detect_target_type(target);

    let process = if is_port {
        let port: u16 = target.parse()?;
        println!(
            "{}",
            format!("🔍 Looking for process on port {}...", port).cyan()
        );
        ProcessService::find_process_by_port(port).await?
    } else {
        let pid: u32 = target.parse()?;
        println!(
            "{}",
            format!("🔍 Looking for process with PID {}...", pid).cyan()
        );
        ProcessService::find_process_by_pid(pid).await?
    };

    match process {
        Some(proc) => {
            if !args.force {
                println!();
                println!("{}", "⚠️  Process to be killed:".yellow().bold());
                println!("  PID:  {}", proc.pid.to_string().magenta());
                println!("  Name: {}", proc.name.cyan());
                println!();
                println!(
                    "{}",
                    "ERROR: Use --force flag to confirm process termination"
                        .red()
                        .bold()
                );
                return Ok(());
            }

            match ProcessService::kill_process(proc.pid).await {
                Ok(_) => {
                    println!();
                    println!("{}", "✅ Process killed successfully!".green().bold());
                    println!("   PID: {}", proc.pid.to_string().magenta());
                    println!("   Name: {}", proc.name.cyan());
                    println!();
                }
                Err(e) => {
                    println!();
                    println!(
                        "{}",
                        format!("❌ Failed to kill process: {}", e).red().bold()
                    );
                    println!();
                }
            }
        }
        None => {
            println!();
            if is_port {
                println!(
                    "{}",
                    format!("❌ No process found listening on port {}", target)
                        .red()
                        .bold()
                );
            } else {
                println!(
                    "{}",
                    format!("❌ Process with PID {} not found", target)
                        .red()
                        .bold()
                );
            }
            println!();
        }
    }

    Ok(())
}

fn detect_target_type(target: &str) -> bool {
    if let Ok(num) = target.parse::<u32>() {
        num <= 65535 && num > 1024
    } else {
        false
    }
}
