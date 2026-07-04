use anyhow::Result;
use colored::Colorize;

use crate::cli::AllEyeArgs;
use crate::core::context::AppContext;
use crate::services::{secret_service::SecretService, storage::SnapshotStore};
use crate::ui::all_eye::{AllEyeData, HealthCheck, HealthStatus};
use crate::utils::terminal;

pub async fn run(ctx: AppContext, _args: AllEyeArgs) -> Result<()> {
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let machine = store.read_machine().await.ok();
    let packages = store.read_packages().await.ok();
    let vscode = store.read_vscode().await.ok();
    let git = store.read_git().await.ok();

    let mut health = Vec::new();
    health.push(HealthCheck {
        label: "Snapshot of the realm".into(),
        status: if machine.is_some() {
            HealthStatus::Ok("captured".into())
        } else {
            HealthStatus::Warn("none — run `odin snapshot`".into())
        },
    });
    health.push(HealthCheck {
        label: "Bifrost (GitHub)".into(),
        status: if ctx.config().github.repository_url.is_some() {
            HealthStatus::Ok("configured".into())
        } else {
            HealthStatus::Warn("not configured".into())
        },
    });
    let platform = &ctx.config().platform;
    health.push(HealthCheck {
        label: "Odin Platform".into(),
        status: if platform.url.is_some() && platform.token_key.is_some() {
            if platform.upload_on_snapshot {
                HealthStatus::Ok("connected · auto-upload".into())
            } else {
                HealthStatus::Ok("connected · manual push".into())
            }
        } else {
            HealthStatus::Warn("not connected — run `odin login`".into())
        },
    });
    if let Some(token_key) = &ctx.config().github.token_key {
        let label = "Rune of authority".to_string();
        let status = if SecretService::get_token(token_key).is_ok() {
            HealthStatus::Ok("token sealed in vault".into())
        } else {
            HealthStatus::Bad("token missing".into())
        };
        health.push(HealthCheck { label, status });
    }
    let pkg_count = packages.as_ref().map(|p| p.packages.len()).unwrap_or(0);
    health.push(HealthCheck {
        label: "Package inventory".into(),
        status: if pkg_count == 0 {
            HealthStatus::Warn("empty".into())
        } else {
            HealthStatus::Ok(format!("{pkg_count} package(s)"))
        },
    });

    if terminal::is_interactive() {
        let data = AllEyeData {
            snapshot_dir: store.root().display().to_string(),
            github_repo: ctx.config().github.repository_url.clone(),
            sync_branch: ctx.config().sync.branch.clone(),
            machine,
            packages,
            vscode,
            git,
            health,
        };
        return crate::ui::all_eye::run(data);
    }

    print_text_fallback(&store, machine, packages, vscode, git, &health);
    Ok(())
}

fn print_text_fallback(
    store: &SnapshotStore,
    machine: Option<crate::models::machine::MachineSnapshot>,
    packages: Option<crate::models::package::PackageSnapshot>,
    vscode: Option<crate::models::vscode::VsCodeExtensionsSnapshot>,
    git: Option<crate::models::git::GitConfigSnapshot>,
    health: &[HealthCheck],
) {
    println!();
    println!(
        "  {}  {}",
        "ᚢ".bright_yellow().bold(),
        "ALL-EYE — the gaze of Odin".bright_white().bold()
    );
    println!("  {}", "═".repeat(54).bright_blue());
    println!(
        "  {}  {}",
        "vault".dimmed(),
        store.root().display().to_string().cyan()
    );

    if let Some(machine) = machine {
        println!();
        println!(
            "  {}",
            "── Hliðskjálf · The High Seat ──".bright_blue().bold()
        );
        println!("    id        {}", machine.snapshot_id.to_string().cyan());
        println!("    captured  {}", machine.captured_at.to_rfc3339());
        println!(
            "    host      {} ({})",
            machine.hostname.cyan(),
            machine.username.dimmed()
        );
        println!("    os        {} {}", machine.os_name, machine.os_version);
        println!("    cpu       {}", machine.cpu_brand);
        println!(
            "    memory    {:.1} GB",
            machine.total_memory_bytes as f64 / 1_073_741_824.0
        );

        println!();
        println!(
            "  {}",
            "── Forges (package managers) ──".bright_blue().bold()
        );
        for manager in machine.package_managers {
            let status = if manager.installed {
                "✓ ready".green()
            } else {
                "· dormant".yellow()
            };
            let version = manager.version.unwrap_or_else(|| "-".to_string());
            println!(
                "    {:8} {:10} {}",
                manager.name.bold(),
                status,
                version.dimmed()
            );
        }
    } else {
        println!();
        println!(
            "  {}",
            "no snapshot in the vault — run `odin snapshot` to capture this realm".yellow()
        );
    }

    println!();
    println!(
        "  {}",
        "── Hugin & Munin (observers) ──".bright_blue().bold()
    );
    for hc in health {
        let (icon, color_status) = match &hc.status {
            HealthStatus::Ok(t) => ("✓".green(), t.green()),
            HealthStatus::Warn(t) => ("!".yellow(), t.yellow()),
            HealthStatus::Bad(t) => ("✗".red(), t.red()),
        };
        println!("    {}  {:24} {}", icon, hc.label, color_status);
    }

    println!();
    println!(
        "  {}",
        "── Hoard (snapshot contents) ──".bright_blue().bold()
    );
    println!(
        "    packages          {}",
        packages
            .map(|p| p.packages.len())
            .unwrap_or_default()
            .to_string()
            .cyan()
    );
    println!(
        "    vscode extensions {}",
        vscode
            .map(|v| v.extensions.len())
            .unwrap_or_default()
            .to_string()
            .cyan()
    );
    println!(
        "    git config        {}",
        git.map(|g| g.entries.len())
            .unwrap_or_default()
            .to_string()
            .cyan()
    );

    println!();
    println!("  {}", "── Runes of summoning ──".bright_blue().bold());
    println!(
        "    odin snapshot         {} capture this realm",
        "→".bright_green()
    );
    println!(
        "    odin doctor           {} divine broken paths",
        "→".bright_green()
    );
    println!(
        "    odin diff             {} compare realm to vault",
        "→".bright_green()
    );
    println!(
        "    odin restore          {} dry-run restore plan",
        "→".bright_green()
    );
    println!(
        "    odin restore --apply  {} bind realm to vault",
        "→".bright_green()
    );
    println!(
        "    odin sync             {} cross the Bifrost",
        "→".bright_green()
    );
    println!(
        "    odin asgard           {} enter the profile realm",
        "→".bright_green()
    );
    println!();
}
