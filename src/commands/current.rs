use anyhow::Result;
use colored::Colorize;

use crate::asgard::store::AsgardStore;
use crate::cli::CurrentArgs;
use crate::core::context::AppContext;
use crate::utils::time;

pub async fn run(ctx: AppContext, args: CurrentArgs) -> Result<()> {
    let store = AsgardStore::new(ctx.odin_dir());
    let state = store.load_state().await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&state)?);
        return Ok(());
    }

    println!();
    println!(
        "  {}  {}",
        "●".bright_yellow().bold(),
        "CURRENT — the bound realm".bright_white().bold()
    );
    println!("  {}", "─".repeat(54).dimmed());

    match (&state.active_profile, &state.activated_at) {
        (Some(name), Some(when)) => {
            println!(
                "  {}  realm    {}",
                "●".green().bold(),
                name.bright_yellow().bold()
            );
            println!(
                "  {}  bound    {} ({})",
                "·".dimmed(),
                time::humanize_since(*when).cyan(),
                when.to_rfc3339().dimmed()
            );
            if let Ok(profile) = store.load(name).await {
                let mut bits: Vec<String> = Vec::new();
                if !profile.env.is_empty() {
                    bits.push(format!("{} runes", profile.env.len()));
                }
                if !profile.startup_apps.is_empty() {
                    bits.push(format!("{} warriors", profile.startup_apps.len()));
                }
                if !profile.browser_urls.is_empty() {
                    bits.push(format!("{} ravens", profile.browser_urls.len()));
                }
                if profile.vscode_workspace.is_some() {
                    bits.push("vscode".to_string());
                }
                if !bits.is_empty() {
                    println!("  {}  bears    {}", "·".dimmed(), bits.join(" · ").cyan());
                }
                if !profile.description.is_empty() {
                    println!(
                        "  {}  scroll   {}",
                        "·".dimmed(),
                        profile.description.italic()
                    );
                }
            }
        }
        _ => {
            println!("  {}  no realm bound", "○".dimmed());
            println!(
                "  {}  enter Asgard with {} to forge or bind one",
                "→".bright_blue(),
                "odin asgard".cyan().bold()
            );
        }
    }

    if !state.recent.is_empty() {
        let parts: Vec<String> = state
            .recent
            .iter()
            .skip(if state.active_profile.is_some() { 1 } else { 0 })
            .map(|e| format!("{} ({})", e.name, time::humanize_since(e.activated_at)))
            .collect();
        if !parts.is_empty() {
            println!("  {}  recent   {}", "·".dimmed(), parts.join(", ").dimmed());
        }
    }
    println!();
    Ok(())
}
