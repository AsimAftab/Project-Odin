use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use comfy_table::Cell;
use std::path::PathBuf;

use crate::core::context::AppContext;
use crate::services::plugin_service::PluginService;
use crate::ui::text_tables::{rule, styled_table};

#[derive(Debug, Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommands,
}

#[derive(Debug, Subcommand)]
pub enum PluginCommands {
    /// Install a plugin by copying a directory containing plugin.toml and its executable.
    Install(PluginInstallArgs),
    /// List installed plugins.
    List(PluginListArgs),
    /// Run a plugin, forwarding any trailing args after `--`.
    Run(PluginRunArgs),
    /// Enable a previously disabled plugin.
    Enable(PluginNameArgs),
    /// Disable a plugin without removing it.
    Disable(PluginNameArgs),
    /// Remove an installed plugin.
    Remove(PluginNameArgs),
}

#[derive(Debug, Args)]
pub struct PluginInstallArgs {
    /// Path to a directory containing plugin.toml and the plugin executable.
    pub source: PathBuf,
}

#[derive(Debug, Args)]
pub struct PluginListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PluginRunArgs {
    pub name: String,
    /// Arguments forwarded to the plugin executable. Use `--` to separate them from Odin flags.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PluginNameArgs {
    pub name: String,
}

pub async fn run(ctx: AppContext, args: PluginArgs) -> Result<()> {
    let service = PluginService::new(ctx.odin_dir());
    match args.command {
        PluginCommands::Install(a) => install(&service, a),
        PluginCommands::List(a) => list(&service, a),
        PluginCommands::Run(a) => run_plugin(&service, a),
        PluginCommands::Enable(a) => set_enabled(&service, &a.name, true),
        PluginCommands::Disable(a) => set_enabled(&service, &a.name, false),
        PluginCommands::Remove(a) => remove(&service, &a.name),
    }
}

fn install(service: &PluginService, args: PluginInstallArgs) -> Result<()> {
    let installed = service.install(&args.source)?;
    println!();
    println!(
        "  {}  plugin {} v{} installed",
        "✓".green().bold(),
        installed.manifest.name.bright_yellow().bold(),
        installed.manifest.version.cyan()
    );
    println!(
        "    {}  {}",
        "location".dimmed(),
        installed.install_path.display().to_string().cyan()
    );
    println!();
    Ok(())
}

fn list(service: &PluginService, args: PluginListArgs) -> Result<()> {
    let plugins = service.list()?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&plugins)?);
        return Ok(());
    }
    println!();
    println!(
        "  {}  {}",
        "ᛚ".bright_yellow().bold(),
        "PLUGINS — runes bound to Odin".bright_white().bold()
    );
    println!("  {}", rule(60).dimmed());
    if plugins.is_empty() {
        println!(
            "  {}  no plugins bound — use {} to install one",
            "·".dimmed(),
            "odin plugin install <dir>".cyan().bold()
        );
        println!();
        return Ok(());
    }
    let mut table = styled_table(&["Name", "Version", "Author", "Status", "Description"]);
    for plugin in &plugins {
        let status = if plugin.enabled {
            "✓ ready".green().to_string()
        } else {
            "· dormant".dimmed().to_string()
        };
        table.add_row(vec![
            Cell::new(&plugin.manifest.name),
            Cell::new(&plugin.manifest.version),
            Cell::new(&plugin.manifest.author),
            Cell::new(status),
            Cell::new(&plugin.manifest.description),
        ]);
    }
    println!("{table}");
    println!();
    Ok(())
}

fn run_plugin(service: &PluginService, args: PluginRunArgs) -> Result<()> {
    let result = service.run(&args.name, &args.args)?;
    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
        if !result.stdout.ends_with('\n') {
            println!();
        }
    }
    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
        if !result.stderr.ends_with('\n') {
            eprintln!();
        }
    }
    if !result.success {
        anyhow::bail!(
            "plugin '{}' exited with code {}",
            args.name,
            result.exit_code
        );
    }
    Ok(())
}

fn set_enabled(service: &PluginService, name: &str, enabled: bool) -> Result<()> {
    service.set_enabled(name, enabled)?;
    println!(
        "  {}  plugin {} {}",
        "✓".green().bold(),
        name.bright_yellow().bold(),
        if enabled {
            "awakened".green().to_string()
        } else {
            "set dormant".dimmed().to_string()
        }
    );
    Ok(())
}

fn remove(service: &PluginService, name: &str) -> Result<()> {
    service.remove(name)?;
    println!(
        "  {}  plugin {} removed",
        "✓".green().bold(),
        name.bright_yellow().bold()
    );
    Ok(())
}
