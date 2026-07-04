use anyhow::Result;
use clap::Args;
use colored::Colorize;
use comfy_table::Cell;
use std::path::PathBuf;

use crate::core::context::AppContext;
use crate::models::watcher::{WatchEventType, WatcherEvent};
use crate::services::platform_service::{self, PlatformService};
use crate::services::snapshot_service::SnapshotService;
use crate::services::storage::SnapshotStore;
use crate::services::watcher_service::WatcherService;
use crate::ui::text_tables::{rule, styled_table};

#[derive(Debug, Args)]
pub struct WatchArgs {
    /// Seconds between samples (default 60).
    #[arg(long, default_value_t = 60)]
    pub interval: u64,
    /// Loop continuously until Ctrl-C; without this, runs one comparison and exits.
    #[arg(long)]
    pub follow: bool,
    /// Append each detected event as a JSON line to this file.
    #[arg(long)]
    pub record: Option<PathBuf>,
    /// Output the first diff as JSON instead of a table (only useful without --follow).
    #[arg(long)]
    pub json: bool,
}

pub async fn run(ctx: AppContext, args: WatchArgs) -> Result<()> {
    let service = WatcherService::new(args.record.clone());

    // Always-on platform sync: when auto-upload is enabled and we're following,
    // each detected drift triggers a fresh snapshot + upload.
    let sync_enabled = args.follow
        && ctx.config().platform.upload_on_snapshot
        && platform_service::is_configured(ctx.config());

    println!();
    println!(
        "  {}  {}",
        "ᛒ".bright_yellow().bold(),
        "WATCH — Hugin & Munin patrol the realm"
            .bright_white()
            .bold()
    );
    println!("  {}", rule(60).dimmed());
    println!(
        "  {}  sampling every {}s{}",
        "·".bright_blue(),
        args.interval.to_string().cyan().bold(),
        if args.follow {
            "  (Ctrl-C to recall)"
        } else {
            "  (one-shot)"
        }
    );
    if let Some(path) = service.record_path() {
        println!(
            "  {}  ravens scribe to {}",
            "·".dimmed(),
            path.display().to_string().cyan()
        );
    }

    if sync_enabled {
        println!(
            "  {}  platform sync on — drift is snapshotted and uploaded",
            "·".bright_blue()
        );
    }

    let mut previous = service.capture()?;
    println!("  {}  initial state captured", "✓".green().bold());

    if !args.follow {
        service.sleep(args.interval).await;
        let current = service.capture()?;
        let events = previous.diff(&current);
        service.record(&events).await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&events)?);
        } else {
            print_events(&events);
        }
        return Ok(());
    }

    loop {
        tokio::select! {
            _ = service.sleep(args.interval) => {}
            _ = tokio::signal::ctrl_c() => {
                println!();
                println!("  {}  ravens recalled", "✓".green().bold());
                return Ok(());
            }
        }
        let current = service.capture()?;
        let events = previous.diff(&current);
        service.record(&events).await?;
        if events.is_empty() {
            println!(
                "  {}  no drift ({})",
                "·".dimmed(),
                chrono::Utc::now().to_rfc3339().dimmed()
            );
        } else {
            print_events(&events);
            if sync_enabled {
                sync_on_drift(&ctx).await;
            }
        }
        previous = current;
    }
}

/// Captures a snapshot and uploads it to the platform. Non-fatal: any failure is
/// reported and the watch loop keeps running.
async fn sync_on_drift(ctx: &AppContext) {
    let store = SnapshotStore::new(ctx.odin_dir().clone());
    let captured = SnapshotService::new(store)
        .with_keep_last(ctx.config().snapshot.keep_last)
        .capture(false, Some("watch".to_string()))
        .await;
    match captured {
        Ok(_) => match PlatformService::new(ctx.odin_dir().clone())
            .upload_latest(ctx.config())
            .await
        {
            Ok(id) => println!(
                "  {}  synced to platform ({})",
                "✓".green().bold(),
                id.bright_yellow()
            ),
            Err(e) => println!(
                "  {}  platform sync failed: {}",
                "⚠".yellow().bold(),
                e.to_string().red()
            ),
        },
        Err(e) => println!(
            "  {}  sync snapshot failed: {}",
            "⚠".yellow().bold(),
            e.to_string().red()
        ),
    }
}

fn print_events(events: &[WatcherEvent]) {
    if events.is_empty() {
        println!("  {}  no drift detected", "·".dimmed());
        return;
    }
    let mut table = styled_table(&["Kind", "Change", "Name", "Detail"]);
    for event in events {
        let (kind, change, name, detail) = render_event(event);
        table.add_row(vec![
            Cell::new(kind),
            Cell::new(change),
            Cell::new(name),
            Cell::new(detail),
        ]);
    }
    println!("{table}");
}

fn render_event(event: &WatcherEvent) -> (&'static str, &'static str, String, String) {
    match event {
        WatcherEvent::EnvVar(e) => {
            let change = match (e.old_value.is_some(), e.new_value.is_some()) {
                (false, true) => "added",
                (true, false) => "removed",
                _ => "modified",
            };
            let detail = match (&e.old_value, &e.new_value) {
                (Some(old), Some(new)) => format!("{} -> {}", truncate(old), truncate(new)),
                (None, Some(new)) => truncate(new),
                (Some(old), None) => truncate(old),
                _ => String::new(),
            };
            ("env", change, e.name.clone(), detail)
        }
        WatcherEvent::Path(p) => {
            let change = match p.change_type {
                WatchEventType::Created => "added",
                WatchEventType::Deleted => "removed",
                WatchEventType::Modified => "modified",
                WatchEventType::Renamed => "renamed",
            };
            ("path", change, p.directory.clone(), String::new())
        }
        WatcherEvent::File(f) => {
            let change = match f.change_type {
                WatchEventType::Created => "added",
                WatchEventType::Deleted => "removed",
                WatchEventType::Modified => "modified",
                WatchEventType::Renamed => "renamed",
            };
            ("file", change, f.path.clone(), String::new())
        }
        WatcherEvent::Package(p) => {
            let change = match p.action.as_str() {
                "install" => "added",
                "remove" => "removed",
                _ => "modified",
            };
            (
                "pkg",
                change,
                format!("{} ({})", p.package_name, p.manager),
                p.version.clone(),
            )
        }
    }
}

fn truncate(s: &str) -> String {
    const LIMIT: usize = 64;
    if s.chars().count() <= LIMIT {
        s.to_string()
    } else {
        let head: String = s.chars().take(LIMIT - 1).collect();
        format!("{head}…")
    }
}
