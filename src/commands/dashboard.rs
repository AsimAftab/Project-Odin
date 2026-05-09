use anyhow::Result;
use colored::Colorize;

use crate::cli::DashboardArgs;
use crate::core::context::AppContext;
use crate::services::{secret_service::SecretService, storage::SnapshotStore};
use crate::ui::dashboard::DashboardData;
use crate::utils::terminal;

pub async fn run(ctx: AppContext, _args: DashboardArgs) -> Result<()> {
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let machine = store.read_machine().await.ok();
    let packages = store.read_packages().await.ok();
    let vscode = store.read_vscode().await.ok();
    let git = store.read_git().await.ok();
    let mut health = Vec::new();
    if machine.is_some() {
        health.push("snapshot available".to_string());
    } else {
        health.push("snapshot missing".to_string());
    }
    if ctx.config().github.repository_url.is_some() {
        health.push("github configured".to_string());
    } else {
        health.push("github not configured".to_string());
    }
    if let Some(token_key) = &ctx.config().github.token_key {
        if SecretService::get_token(token_key).is_ok() {
            health.push("github token available".to_string());
        } else {
            health.push("github token missing".to_string());
        }
    }
    if packages
        .as_ref()
        .map(|p| p.packages.is_empty())
        .unwrap_or(true)
    {
        health.push("package inventory empty".to_string());
    } else {
        health.push("package inventory captured".to_string());
    }

    if terminal::is_interactive() {
        let data = DashboardData {
            snapshot_dir: store.root().display().to_string(),
            github_repo: ctx.config().github.repository_url.clone(),
            sync_branch: ctx.config().sync.branch.clone(),
            machine,
            packages,
            vscode,
            git,
            health,
        };
        return crate::ui::dashboard::run(data);
    }

    println!("{}", "Odin Developer Environment".bold());
    println!("{} {}", "snapshot dir".cyan(), store.root().display());

    if let Some(machine) = machine {
        println!();
        println!("{}", "Latest Snapshot".bold());
        println!("  id        {}", machine.snapshot_id);
        println!("  captured  {}", machine.captured_at);
        println!("  host      {} ({})", machine.hostname, machine.username);
        println!("  os        {} {}", machine.os_name, machine.os_version);
        println!("  cpu       {}", machine.cpu_brand);
        println!(
            "  memory    {:.1} GB",
            machine.total_memory_bytes as f64 / 1_073_741_824.0
        );

        println!();
        println!("{}", "Package Managers".bold());
        for manager in machine.package_managers {
            let status = if manager.installed {
                "installed".green()
            } else {
                "missing".yellow()
            };
            let version = manager.version.unwrap_or_else(|| "-".to_string());
            println!("  {:8} {:10} {}", manager.name, status, version);
        }
    } else {
        println!();
        println!(
            "{}",
            "No snapshot found. Run `odin snapshot` first.".yellow()
        );
    }

    println!();
    println!("{}", "Snapshot Contents".bold());
    println!(
        "  packages          {}",
        packages.map(|p| p.packages.len()).unwrap_or_default()
    );
    println!(
        "  vscode extensions {}",
        vscode.map(|v| v.extensions.len()).unwrap_or_default()
    );
    println!(
        "  git config        {}",
        git.map(|g| g.entries.len()).unwrap_or_default()
    );

    println!();
    println!("{}", "Commands".bold());
    println!("  odin snapshot        capture current workstation");
    println!("  odin doctor          diagnose broken paths/tools");
    println!("  odin diff            compare live machine to snapshot");
    println!("  odin restore         dry-run restore plan");
    println!("  odin restore --apply apply restore plan");
    println!("  odin sync            commit and push snapshot state");
    Ok(())
}
