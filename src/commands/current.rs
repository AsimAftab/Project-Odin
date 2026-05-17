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

    match (&state.active_profile, &state.activated_at) {
        (Some(name), Some(when)) => {
            println!("{} {}", "Active Profile:".bold(), name.cyan());
            println!(
                "{}      {} ({})",
                "Activated:".bold(),
                time::humanize_since(*when),
                when.to_rfc3339()
            );
        }
        _ => {
            println!("{} no active profile", "·".dimmed());
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
            println!("{}         {}", "Recent:".bold(), parts.join(", "));
        }
    }
    Ok(())
}
