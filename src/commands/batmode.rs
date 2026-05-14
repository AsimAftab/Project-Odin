use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use comfy_table::Cell;

use crate::core::context::AppContext;
use crate::models::batmode::BatmodeEntry;
use crate::services::batmode_service::BatmodeService;
use crate::ui::text_tables::{rule, styled_table};

#[derive(Debug, Args)]
pub struct BatmodeArgs {
    #[command(subcommand)]
    pub command: BatmodeCommands,
}

#[derive(Debug, Subcommand)]
pub enum BatmodeCommands {
    /// Add an executable to a profile (creates the profile if needed).
    Add(BatmodeAddArgs),
    /// Remove an entry by index, or an entire profile with --all.
    Remove(BatmodeRemoveArgs),
    /// List all profiles with entry counts.
    List(BatmodeListArgs),
    /// Launch every entry in a profile in detached child processes.
    Launch(BatmodeLaunchArgs),
    /// Print the entries of a single profile.
    Show(BatmodeShowArgs),
}

#[derive(Debug, Args)]
pub struct BatmodeAddArgs {
    /// Profile name (e.g. "work", "study").
    pub profile: String,
    /// Path to the executable.
    pub path: String,
    /// Optional command-line arguments to pass to the executable.
    #[arg(long, value_parser, num_args = 0..)]
    pub args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct BatmodeRemoveArgs {
    /// Profile name.
    pub profile: String,
    /// Index of the entry to remove (use `list`/`show` to see indices).
    #[arg(long)]
    pub index: Option<usize>,
    /// Remove the entire profile.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct BatmodeListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct BatmodeLaunchArgs {
    /// Profile name.
    pub profile: String,
}

#[derive(Debug, Args)]
pub struct BatmodeShowArgs {
    /// Profile name.
    pub profile: String,
    #[arg(long)]
    pub json: bool,
}

pub async fn run(ctx: AppContext, args: BatmodeArgs) -> Result<()> {
    let service = BatmodeService::new(ctx.odin_dir());
    match args.command {
        BatmodeCommands::Add(a) => add(&service, a),
        BatmodeCommands::Remove(a) => remove(&service, a),
        BatmodeCommands::List(a) => list(&service, a),
        BatmodeCommands::Launch(a) => launch(&service, a),
        BatmodeCommands::Show(a) => show(&service, a),
    }
}

fn add(service: &BatmodeService, args: BatmodeAddArgs) -> Result<()> {
    let entry = BatmodeEntry {
        path: args.path.clone(),
        args: args.args,
    };
    service.add(&args.profile, entry.clone())?;
    println!(
        "{} added {} to profile {}",
        "ok".green(),
        entry.display_name().bright_yellow(),
        args.profile.cyan()
    );
    Ok(())
}

fn remove(service: &BatmodeService, args: BatmodeRemoveArgs) -> Result<()> {
    if args.all {
        service.remove_profile(&args.profile)?;
        println!("{} removed profile {}", "ok".green(), args.profile.cyan());
        return Ok(());
    }
    let Some(index) = args.index else {
        anyhow::bail!("specify --index <N> or --all");
    };
    let removed = service.remove_entry(&args.profile, index)?;
    println!(
        "{} removed {} from profile {}",
        "ok".green(),
        removed.display_name().bright_yellow(),
        args.profile.cyan()
    );
    Ok(())
}

fn list(service: &BatmodeService, args: BatmodeListArgs) -> Result<()> {
    let config = service.load()?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }
    println!("{}", "Batmode Profiles".bold().cyan());
    println!("{}\n", rule(60));
    if config.profiles.is_empty() {
        println!(
            "{} no profiles configured. Use {} to add one.",
            "info".blue(),
            "odin batmode add <profile> <path>".cyan()
        );
        return Ok(());
    }
    let mut table = styled_table(&["Profile", "Entries", "Apps"]);
    for (name, entries) in &config.profiles {
        let preview = entries
            .iter()
            .take(3)
            .map(BatmodeEntry::display_name)
            .collect::<Vec<_>>()
            .join(", ");
        let extra = if entries.len() > 3 {
            format!(", +{} more", entries.len() - 3)
        } else {
            String::new()
        };
        table.add_row(vec![
            Cell::new(name),
            Cell::new(entries.len()),
            Cell::new(format!("{preview}{extra}")),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn show(service: &BatmodeService, args: BatmodeShowArgs) -> Result<()> {
    let config = service.load()?;
    let entries = config
        .profiles
        .get(&args.profile)
        .ok_or_else(|| anyhow::anyhow!("profile '{}' not found", args.profile))?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(entries)?);
        return Ok(());
    }
    println!(
        "{} {}",
        "Profile".bold().cyan(),
        args.profile.bright_yellow()
    );
    println!("{}\n", rule(60));
    if entries.is_empty() {
        println!("(no entries)");
        return Ok(());
    }
    let mut table = styled_table(&["#", "Path", "Args"]);
    for (idx, entry) in entries.iter().enumerate() {
        table.add_row(vec![
            Cell::new(idx),
            Cell::new(&entry.path),
            Cell::new(entry.args.join(" ")),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn launch(service: &BatmodeService, args: BatmodeLaunchArgs) -> Result<()> {
    let config = service.load()?;
    let entries = config
        .profiles
        .get(&args.profile)
        .ok_or_else(|| anyhow::anyhow!("profile '{}' not found", args.profile))?
        .clone();
    println!(
        "{} launching profile {}",
        "->".cyan(),
        args.profile.bright_yellow()
    );
    for entry in &entries {
        println!("  {} starting {}...", "->".cyan(), entry.display_name());
    }
    let summary = service.launch(&args.profile)?;
    for failure in &summary.failures {
        println!("  {} {}", "fail".red(), failure);
    }
    println!(
        "{} launched {}/{}",
        if summary.failures.is_empty() {
            "ok".green()
        } else {
            "partial".yellow()
        },
        summary.launched,
        summary.total
    );
    Ok(())
}
