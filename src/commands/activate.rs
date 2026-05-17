use std::path::Path;

use anyhow::{bail, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::asgard::profile::RESERVED_NAME;
use crate::asgard::store::AsgardStore;
use crate::cli::ActivateArgs;
use crate::core::context::AppContext;
use crate::services::asgard_service;
use crate::ui::asgard::{self as asgard_ui, AsgardAction};
use crate::utils::terminal;

pub async fn run(ctx: AppContext, args: ActivateArgs) -> Result<()> {
    let odin_dir = ctx.odin_dir().clone();
    let store = AsgardStore::new(&odin_dir);
    store.ensure().await?;

    let interactive = terminal::is_interactive() && !args.non_interactive;

    let target = args.name.as_deref();
    let opens_tui = match target {
        Some(n) if n.eq_ignore_ascii_case(RESERVED_NAME) => true,
        Some(_) => false,
        None => interactive,
    };

    if opens_tui {
        if !interactive {
            bail!(
                "interactive selector requires a TTY; pass a profile name or drop --non-interactive"
            );
        }
        return run_tui(&odin_dir, args.json).await;
    }

    let name = target.expect("checked above").to_string();
    activate_named(&odin_dir, &name, args.json).await
}

async fn run_tui(odin_dir: &Path, json: bool) -> Result<()> {
    let store = AsgardStore::new(odin_dir);
    let summaries = store.list_summaries().await?;

    if summaries.is_empty() {
        println!("{}", "No profiles found in Asgard.".bold());
        println!("Let's create your first one.\n");
        let profile = asgard_service::wizard(odin_dir, None).await?;
        if asgard_service::confirm("Activate it now?", true)? {
            return activate_named(odin_dir, &profile.name, json).await;
        }
        return Ok(());
    }

    let active = store.load_state().await?.active_profile;
    let action = asgard_ui::run(summaries, active)?;
    match action {
        AsgardAction::Activate(name) => activate_named(odin_dir, &name, json).await,
        AsgardAction::Create => {
            let profile = asgard_service::wizard(odin_dir, None).await?;
            if asgard_service::confirm("Activate it now?", true)? {
                activate_named(odin_dir, &profile.name, json).await
            } else {
                Ok(())
            }
        }
        AsgardAction::Edit(name) => {
            asgard_service::edit_interactive(odin_dir, &name).await?;
            Ok(())
        }
        AsgardAction::Delete(name) => {
            asgard_service::delete(odin_dir, &name).await?;
            println!("{} deleted profile {}", "ok".green(), name.cyan());
            Ok(())
        }
        AsgardAction::Quit => Ok(()),
    }
}

async fn activate_named(odin_dir: &Path, name: &str, json: bool) -> Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("{spinner:.cyan} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner.set_message(format!("activating {name}"));

    let report = asgard_service::activate(odin_dir, name).await?;
    spinner.finish_and_clear();

    if json {
        let payload = serde_json::json!({
            "profile": report.profile,
            "started": report.started,
            "failed": report.failed
                .iter()
                .map(|(label, err)| serde_json::json!({"target": label, "error": err}))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        asgard_service::print_activation_report(&report);
    }
    Ok(())
}
