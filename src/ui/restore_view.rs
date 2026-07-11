//! Terminal rendering for restore plans and reports (doctor-style tables).

use std::collections::BTreeMap;

use colored::Colorize;
use comfy_table::{Cell, Color};

use crate::models::restore::{
    manager_label, InstallOutcome, PlanAction, RestorePlan, RestoreReport,
};
use crate::ui::text_tables::{rule, styled_table};

#[derive(Default)]
struct ManagerCounts {
    install: usize,
    present: usize,
    skipped: usize,
    missing: usize,
    no_command: usize,
}

fn plan_counts(plan: &RestorePlan) -> BTreeMap<&'static str, ManagerCounts> {
    let mut by_manager: BTreeMap<&'static str, ManagerCounts> = BTreeMap::new();
    for p in &plan.packages {
        let entry = by_manager.entry(manager_label(&p.source)).or_default();
        match p.action {
            PlanAction::WillInstall => entry.install += 1,
            PlanAction::AlreadyInstalled => entry.present += 1,
            PlanAction::DisabledByConfig | PlanAction::ExcludedByUser => entry.skipped += 1,
            PlanAction::ManagerMissing => entry.missing += 1,
            PlanAction::NoInstallCommand => entry.no_command += 1,
        }
    }
    by_manager
}

/// Renders the pre-execution plan: what each section will do, per-manager
/// package counts, and everything that needs attention.
pub fn print_plan(plan: &RestorePlan) {
    // Sections.
    let mut sections = styled_table(&["SECTION", "STATUS", "ITEMS"]);
    for s in &plan.sections {
        let status = if s.enabled {
            Cell::new("restore").fg(Color::Green)
        } else {
            Cell::new(format!("skip ({})", s.reason)).fg(Color::DarkGrey)
        };
        sections.add_row(vec![
            Cell::new(s.section.label()),
            status,
            Cell::new(s.item_count),
        ]);
    }
    println!("{sections}");

    // Per-manager package counts (only when packages are in play).
    let by_manager = plan_counts(plan);
    if !by_manager.is_empty() {
        let mut table = styled_table(&[
            "MANAGER", "INSTALL", "PRESENT", "SKIPPED", "MISSING", "NO CMD",
        ]);
        for (manager, c) in &by_manager {
            table.add_row(vec![
                Cell::new(*manager),
                Cell::new(c.install).fg(if c.install > 0 {
                    Color::Green
                } else {
                    Color::DarkGrey
                }),
                Cell::new(c.present).fg(Color::DarkGrey),
                Cell::new(c.skipped).fg(Color::DarkGrey),
                Cell::new(c.missing).fg(if c.missing > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGrey
                }),
                Cell::new(c.no_command).fg(if c.no_command > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGrey
                }),
            ]);
        }
        println!("{table}");
    }

    // Attention list: what --apply will NOT be able to do.
    let attention: Vec<_> = plan
        .packages
        .iter()
        .filter(|p| {
            matches!(
                p.action,
                PlanAction::ManagerMissing | PlanAction::NoInstallCommand
            )
        })
        .collect();
    if !attention.is_empty() {
        println!(
            "  {}  {} package(s) will need attention:",
            "!".yellow().bold(),
            attention.len().to_string().yellow().bold()
        );
        for p in attention.iter().take(15) {
            let why = match p.action {
                PlanAction::ManagerMissing => {
                    format!("{} not installed", manager_label(&p.source))
                }
                _ => "no install command".to_string(),
            };
            println!("     {} {} ({})", "·".dimmed(), p.id.cyan(), why.dimmed());
        }
        if attention.len() > 15 {
            println!("     {} … and {} more", "·".dimmed(), attention.len() - 15);
        }
    }
    if !plan.missing_managers.is_empty() {
        let labels: Vec<&str> = plan.missing_managers.iter().map(manager_label).collect();
        println!(
            "  {}  missing manager(s): {} — {} can install them",
            "!".yellow().bold(),
            labels.join(", ").cyan(),
            "--bootstrap-managers".cyan()
        );
    }
}

/// Renders the post-apply report: per-manager outcome table, section results,
/// the MANUAL INSTALL REQUIRED table, and a colored footer.
pub fn print_report(report: &RestoreReport) {
    #[derive(Default)]
    struct Row {
        installed: usize,
        present: usize,
        skipped: usize,
        failed: usize,
        unavailable: usize,
        manual_other: usize,
    }
    let mut by_manager: BTreeMap<&'static str, Row> = BTreeMap::new();
    for p in &report.packages {
        let row = by_manager.entry(manager_label(&p.source)).or_default();
        match &p.outcome {
            InstallOutcome::Installed => row.installed += 1,
            InstallOutcome::AlreadyInstalled => row.present += 1,
            InstallOutcome::Skipped { .. } => row.skipped += 1,
            InstallOutcome::Failed { .. } => row.failed += 1,
            InstallOutcome::UnavailableInManager => row.unavailable += 1,
            InstallOutcome::ManagerMissing | InstallOutcome::NoInstallCommand => {
                row.manual_other += 1
            }
        }
    }

    if !by_manager.is_empty() {
        let mut table = styled_table(&[
            "MANAGER",
            "INSTALLED",
            "PRESENT",
            "SKIPPED",
            "FAILED",
            "UNAVAILABLE",
            "MANUAL",
        ]);
        for (manager, row) in &by_manager {
            table.add_row(vec![
                Cell::new(*manager),
                Cell::new(row.installed).fg(if row.installed > 0 {
                    Color::Green
                } else {
                    Color::DarkGrey
                }),
                Cell::new(row.present).fg(Color::DarkGrey),
                Cell::new(row.skipped).fg(Color::DarkGrey),
                Cell::new(row.failed).fg(if row.failed > 0 {
                    Color::Red
                } else {
                    Color::DarkGrey
                }),
                Cell::new(row.unavailable).fg(if row.unavailable > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGrey
                }),
                Cell::new(row.manual_other).fg(if row.manual_other > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGrey
                }),
            ]);
        }
        println!("{table}");
    }

    // One line per non-package section.
    let sections = [
        ("extensions", &report.extensions),
        ("git", &report.git),
        ("env", &report.environment),
        ("PATH", &report.path),
        ("terminal", &report.terminal),
        ("ps-profile", &report.ps_profile),
        ("vscode-settings", &report.vscode_settings),
    ];
    let summary: Vec<String> = sections
        .iter()
        .map(|(name, r)| {
            if r.attempted == 0 {
                format!("{name} —")
            } else if r.failed == 0 {
                format!("{name} {}/{}", r.succeeded, r.attempted)
            } else {
                format!(
                    "{name} {}/{} ({} failed)",
                    r.succeeded, r.attempted, r.failed
                )
            }
        })
        .collect();
    println!("  {}  {}", "·".dimmed(), summary.join(" · ").dimmed());

    if !report.bootstrapped_managers.is_empty() {
        println!(
            "  {}  bootstrapped: {}",
            "✓".green().bold(),
            report.bootstrapped_managers.join(", ").cyan()
        );
    }

    // The centerpiece: everything the user must handle themselves.
    if !report.manual.is_empty() {
        println!();
        println!(
            "  {}",
            format!(
                "──────── MANUAL INSTALL REQUIRED ({}) ────────",
                report.manual.len()
            )
            .yellow()
            .bold()
        );
        let mut table = styled_table(&["PACKAGE", "VERSION", "SOURCE", "REASON", "HINT"]);
        for item in &report.manual {
            table.add_row(vec![
                Cell::new(&item.name),
                Cell::new(item.version.as_deref().unwrap_or("—")),
                Cell::new(manager_label(&item.source)),
                Cell::new(&item.reason).fg(Color::Yellow),
                Cell::new(item.hint.as_deref().unwrap_or("—")).fg(Color::Cyan),
            ]);
        }
        println!("{table}");
    }

    // Footer.
    let installed = report.installed_count();
    let failed = report.failed_count();
    let manual = report.manual.len();
    println!("  {}", rule(60).dimmed());
    let mut parts: Vec<String> = Vec::new();
    if failed > 0 {
        parts.push(format!("{} {} failed", "✗".red().bold(), failed));
    }
    if manual > 0 {
        parts.push(format!("{} {} manual", "!".yellow().bold(), manual));
    }
    parts.push(format!("{} {} installed", "✓".green().bold(), installed));
    println!("  {}", parts.join(" · "));
}
