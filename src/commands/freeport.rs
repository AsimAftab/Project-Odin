use anyhow::Result;
use colored::Colorize;

use crate::cli::FreeportArgs;
use crate::services::process_service::ProcessService;

pub async fn run(_ctx: crate::core::context::AppContext, args: FreeportArgs) -> Result<()> {
    print_banner();

    let target = &args.target;
    let is_port = detect_target_type(target);

    let process = if is_port {
        let port: u16 = target.parse()?;
        println!(
            "  {}  scrying port {}",
            "ᚱ".bright_yellow(),
            port.to_string().cyan().bold()
        );
        ProcessService::find_process_by_port(port).await?
    } else {
        let pid: u32 = target.parse()?;
        println!(
            "  {}  scrying PID {}",
            "ᚱ".bright_yellow(),
            pid.to_string().cyan().bold()
        );
        ProcessService::find_process_by_pid(pid).await?
    };

    match process {
        Some(proc) => {
            if !args.force {
                println!();
                println!(
                    "  {} {}",
                    "⚠".yellow().bold(),
                    "Bound process detected".yellow().bold()
                );
                println!(
                    "    {}  {}",
                    "PID".dimmed(),
                    proc.pid.to_string().magenta().bold()
                );
                println!("    {} {}", "name".dimmed(), proc.name.cyan().bold());
                println!();
                println!(
                    "  {} pass {} to release this binding",
                    "→".bright_blue(),
                    "--force".bright_red().bold()
                );
                println!();
                return Ok(());
            }

            match ProcessService::kill_process(proc.pid).await {
                Ok(_) => {
                    println!();
                    println!(
                        "  {}  {}",
                        "✓".green().bold(),
                        "Port freed by Mjölnir".green().bold()
                    );
                    println!("    {}  {}", "PID".dimmed(), proc.pid.to_string().magenta());
                    println!("    {} {}", "name".dimmed(), proc.name.cyan());
                    println!();
                }
                Err(e) => {
                    println!();
                    println!(
                        "  {}  {} — {}",
                        "✗".red().bold(),
                        "Bifrost shattered".red().bold(),
                        e.to_string().dimmed()
                    );
                    println!();
                }
            }
        }
        None => {
            println!();
            if is_port {
                println!(
                    "  {}  no realm holds port {}",
                    "·".dimmed(),
                    target.cyan().bold()
                );
            } else {
                println!(
                    "  {}  no process answers to PID {}",
                    "·".dimmed(),
                    target.cyan().bold()
                );
            }
            println!();
        }
    }

    Ok(())
}

fn print_banner() {
    println!();
    println!(
        "  {}  {}",
        "⚒".bright_yellow().bold(),
        "FREEPORT — sever a binding, release the realm"
            .bright_white()
            .bold()
    );
    println!("  {}", "Mjölnir is wielded with care.".dimmed());
    println!("  {}", "─".repeat(60).dimmed());
}

fn detect_target_type(target: &str) -> bool {
    if let Ok(num) = target.parse::<u32>() {
        num <= 65535 && num > 1024
    } else {
        false
    }
}
