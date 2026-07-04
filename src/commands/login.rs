use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use crate::cli::LoginArgs;
use crate::core::context::AppContext;
use crate::services::config_service::ConfigService;
use crate::services::platform_service::{self, PlatformService};
use crate::utils::terminal;

pub async fn run(ctx: AppContext, args: LoginArgs) -> Result<()> {
    let interactive = terminal::is_interactive() && !args.non_interactive;
    let url = args.url.clone();
    let service = PlatformService::new(ctx.odin_dir().clone());

    // Already connected? Verify the stored token and short-circuit unless the
    // user explicitly wants to reconnect / switch accounts.
    if !args.force && platform_service::is_configured(ctx.config()) {
        if let Ok(identity) = service.verify(ctx.config()).await {
            let who = identity
                .email
                .or(identity.name)
                .unwrap_or_else(|| "your account".to_string());
            println!();
            println!(
                "  {}  Already connected as {}",
                "✓".green().bold(),
                who.bright_yellow().bold()
            );
            let reconnect = interactive
                && Confirm::new()
                    .with_prompt("Reconnect / switch account?")
                    .default(false)
                    .interact()?;
            if !reconnect {
                println!(
                    "  {}  you're all set — {} to upload, {} to disconnect",
                    "·".dimmed(),
                    "odin sync".cyan(),
                    "odin logout".cyan()
                );
                println!();
                return Ok(());
            }
        }
        // Token invalid/unreachable → fall through to a fresh login.
    }

    let result = service
        .login(&url, hostname().as_deref(), !args.no_browser)
        .await?;

    println!();
    match &result.email {
        Some(email) => println!(
            "  {}  Connected to {} as {}",
            "✓".green().bold(),
            result.url.cyan(),
            email.bright_yellow().bold()
        ),
        None => println!(
            "  {}  Connected to {}",
            "✓".green().bold(),
            result.url.cyan()
        ),
    }

    // Consent — automatic upload of future snapshots.
    let auto_upload = if args.auto_upload || args.yes {
        true
    } else if interactive {
        Confirm::new()
            .with_prompt("Automatically upload each new snapshot to the platform?")
            .default(true)
            .interact()?
    } else {
        false
    };
    service.set_upload_on_snapshot(auto_upload).await?;

    // Consent — backfill existing local snapshots now.
    let backfill = if args.push_existing || args.yes {
        true
    } else if interactive {
        Confirm::new()
            .with_prompt("Upload your existing local snapshots now?")
            .default(true)
            .interact()?
    } else {
        false
    };

    if backfill {
        let config = ConfigService::new(ctx.odin_dir().clone()).load().await?;
        let summary = service.upload_all_history(&config).await?;
        if summary.total == 0 {
            println!("  {}  no local snapshots to upload yet", "·".dimmed());
        } else {
            let failed = if summary.failed > 0 {
                format!(" ({} failed)", summary.failed).red().to_string()
            } else {
                String::new()
            };
            println!(
                "  {}  uploaded {}/{} snapshots{}",
                "✓".green().bold(),
                summary.uploaded,
                summary.total,
                failed
            );
        }
    }

    println!();
    if auto_upload {
        println!(
            "  {}  {} now pushes automatically; {} syncs on drift",
            "→".bright_blue(),
            "odin snapshot".cyan(),
            "odin watch --follow".cyan()
        );
    } else {
        println!(
            "  {}  run {} to upload snapshots",
            "→".bright_blue(),
            "odin push".cyan().bold()
        );
    }
    println!();
    Ok(())
}

/// Device label shown on the approval page. Best-effort machine name.
fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
}
