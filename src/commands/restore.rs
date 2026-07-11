use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{Confirm, MultiSelect};

use crate::cli::RestoreArgs;
use crate::core::context::AppContext;
use crate::models::restore::{manager_label, RestoreReport, RestoreSection};
use crate::services::platform_service::PlatformService;
use crate::services::restore_service::{RestoreInputs, RestoreOptions, RestoreService};
use crate::ui::restore_view;
use crate::utils::{fs as odin_fs, terminal};

pub async fn run(ctx: AppContext, args: RestoreArgs) -> Result<()> {
    if args.interactive && !terminal::is_interactive() {
        anyhow::bail!(
            "--interactive requires a TTY; drop the flag or use --only/--skip/--managers instead"
        );
    }

    let mut options = RestoreOptions::resolve(&ctx.config().restore, &args);
    let service = RestoreService::new(crate::services::storage::SnapshotStore::new(
        ctx.odin_dir().clone(),
    ));

    if !options.quiet {
        println!();
        let title = if args.apply {
            "RESTORE — bind realm to vault"
        } else {
            "RESTORE — plan the binding (dry-run)"
        };
        println!(
            "  {}  {}",
            "ᛞ".bright_yellow().bold(),
            title.bright_white().bold()
        );
        println!("  {}", "─".repeat(60).dimmed());
    }

    // Load the four sections from wherever the snapshot lives. Local ids go
    // through the history index resolver (exact id, tag, or git-style
    // unambiguous prefix); anything not found locally is fetched from the
    // platform, which also accepts short-id prefixes.
    let local_id = args.snapshot.as_deref().and_then(|id| {
        crate::services::history_service::HistoryService::new(ctx.odin_dir().clone())
            .resolve(id)
            .ok()
            .filter(|resolved| service.has_local_history(resolved))
    });
    let (packages, environment, vscode, git) = match (&args.snapshot, &local_id) {
        (None, _) => service.load_vault().await?,
        (Some(_), Some(resolved)) => service.load_history(resolved).await?,
        (Some(id), None) => {
            if !options.quiet {
                println!(
                    "  {}  no local history for {} — checking the Odin Platform…",
                    "·".dimmed(),
                    id.cyan()
                );
            }
            PlatformService::new(ctx.odin_dir().clone())
                .fetch_snapshot(ctx.config(), id)
                .await
                .with_context(|| format!("snapshot {id} not found locally or on the platform"))?
        }
    };
    let inputs = RestoreInputs {
        packages: &packages,
        environment: &environment,
        vscode: &vscode,
        git: &git,
    };

    // Interactive pickers: flags pre-seed the defaults, the picker finalizes.
    if args.interactive {
        pick_sections(&mut options, &inputs)?;
        if options.section_enabled(RestoreSection::Packages) {
            pick_managers(&mut options, &inputs)?;
        }
    }

    let plan = service.plan(&inputs, &options).await?;

    // Dry-run: show the plan and stop.
    if !args.apply {
        let report = RestoreReport::dry_run(args.snapshot.as_deref(), plan);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }
        restore_view::print_plan(&report.plan);
        let will = report
            .plan
            .count(&crate::models::restore::PlanAction::WillInstall);
        println!();
        println!(
            "  {}  plan only — re-run with {} to execute ({} to install)",
            "·".dimmed(),
            "--apply".cyan().bold(),
            will.to_string().cyan().bold()
        );
        println!();
        return Ok(());
    }

    // Apply: interactive mode shows the plan and asks once before executing.
    if args.interactive {
        restore_view::print_plan(&plan);
        println!();
        let go = Confirm::new()
            .with_prompt("Execute this plan?")
            .default(true)
            .interact()?;
        if !go {
            println!("  {}  aborted — nothing was changed", "·".dimmed());
            println!();
            return Ok(());
        }
    }

    let report = service
        .execute(plan, &inputs, &options, args.snapshot.as_deref())
        .await?;

    // Persist the full report next to the vault.
    let report_path = ctx.odin_dir().join("logs").join(format!(
        "restore-{}.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%S")
    ));
    odin_fs::write_json(&report_path, &report).await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!();
        restore_view::print_report(&report);
        println!(
            "  {}  full report: {}",
            "·".dimmed(),
            report_path.display().to_string().dimmed()
        );
        println!();
    }

    if report.has_failures() {
        anyhow::bail!("restore completed with failures — see the report above");
    }
    Ok(())
}

/// MultiSelect over sections, pre-seeded from the flag-resolved options.
fn pick_sections(options: &mut RestoreOptions, inputs: &RestoreInputs<'_>) -> Result<()> {
    let env_count = inputs
        .environment
        .user_variables
        .iter()
        .filter(|v| !v.name.eq_ignore_ascii_case("PATH"))
        .count();
    let profile_count = |p: &Option<crate::models::environment::ProfileSnapshot>| {
        usize::from(p.as_ref().is_some_and(|s| !s.content.is_empty()))
    };
    let counts = [
        inputs.packages.packages.len(),
        inputs.vscode.extensions.len(),
        inputs.git.entries.len(),
        env_count,
        inputs.environment.path_entries.len(),
        profile_count(&inputs.environment.terminal_settings),
        profile_count(&inputs.environment.powershell_profile),
        profile_count(&inputs.vscode.settings)
            + profile_count(&inputs.vscode.keybindings)
            + inputs.vscode.snippets.len(),
    ];
    let items: Vec<String> = RestoreSection::ALL
        .iter()
        .zip(counts)
        .map(|(s, n)| format!("{} ({})", s.label(), n))
        .collect();
    let defaults: Vec<bool> = RestoreSection::ALL
        .iter()
        .map(|s| options.section_enabled(*s))
        .collect();

    let picked = MultiSelect::new()
        .with_prompt("Sections to restore (space toggles, enter confirms)")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    let sections: Vec<RestoreSection> =
        picked.into_iter().map(|i| RestoreSection::ALL[i]).collect();
    options.set_sections(sections);
    Ok(())
}

/// MultiSelect over the managers present in the snapshot, pre-seeded from the
/// flag/config manager list.
fn pick_managers(options: &mut RestoreOptions, inputs: &RestoreInputs<'_>) -> Result<()> {
    // Count packages per manager present in the snapshot.
    let mut counts: Vec<(crate::models::package::PackageManager, usize)> = Vec::new();
    for p in &inputs.packages.packages {
        if matches!(
            p.source,
            crate::models::package::PackageManager::Manual
                | crate::models::package::PackageManager::Unknown
        ) {
            continue;
        }
        if let Some(entry) = counts.iter_mut().find(|(m, _)| *m == p.source) {
            entry.1 += 1;
        } else {
            counts.push((p.source.clone(), 1));
        }
    }
    if counts.is_empty() {
        return Ok(());
    }

    let items: Vec<String> = counts
        .iter()
        .map(|(m, n)| format!("{} ({})", manager_label(m), n))
        .collect();
    let defaults: Vec<bool> = counts
        .iter()
        .map(|(m, _)| crate::services::restore_service::source_enabled(m, &options.managers))
        .collect();

    let picked = MultiSelect::new()
        .with_prompt("Package managers to restore from")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    options.managers = picked
        .into_iter()
        .map(|i| manager_label(&counts[i].0).to_string())
        .collect();
    Ok(())
}
