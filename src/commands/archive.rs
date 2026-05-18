use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use crate::core::context::AppContext;
use crate::services::archive_service::ArchiveService;
use crate::ui::text_tables::rule;

#[derive(Debug, Args)]
pub struct ArchiveArgs {
    #[command(subcommand)]
    pub command: ArchiveCommands,
}

#[derive(Debug, Subcommand)]
pub enum ArchiveCommands {
    /// Bundle a directory into a tar.gz file.
    Create(ArchiveCreateArgs),
    /// Extract a tar.gz file into a directory.
    Extract(ArchiveExtractArgs),
    /// Export a historical snapshot (by ID or tag) into a shareable tar.gz bundle.
    Export(ArchiveExportArgs),
    /// Import a previously exported snapshot bundle, registering it in history.
    Import(ArchiveImportArgs),
}

#[derive(Debug, Args)]
pub struct ArchiveCreateArgs {
    /// Directory to bundle.
    pub input_dir: PathBuf,
    /// Output .tar.gz file.
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct ArchiveExtractArgs {
    /// Input .tar.gz file.
    pub input: PathBuf,
    /// Directory to extract into (created if missing).
    pub output_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct ArchiveExportArgs {
    /// Snapshot ID or tag to export.
    pub snapshot: String,
    /// Output bundle path.
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct ArchiveImportArgs {
    /// Bundle file to import.
    pub input: PathBuf,
}

pub async fn run(ctx: AppContext, args: ArchiveArgs) -> Result<()> {
    let service = ArchiveService::new(ctx.odin_dir().clone());
    println!();
    println!(
        "  {}  {}",
        "ᚹ".bright_yellow().bold(),
        "ARCHIVE — runes bundled for travel".bright_white().bold()
    );
    println!("  {}", rule(60).dimmed());
    match args.command {
        ArchiveCommands::Create(a) => {
            service.create(&a.input_dir, &a.output)?;
            println!(
                "  {}  bundled {} → {}",
                "✓".green().bold(),
                a.input_dir.display().to_string().cyan(),
                a.output.display().to_string().bright_yellow().bold()
            );
        }
        ArchiveCommands::Extract(a) => {
            service.extract(&a.input, &a.output_dir)?;
            println!(
                "  {}  unfurled {} → {}",
                "✓".green().bold(),
                a.input.display().to_string().cyan(),
                a.output_dir.display().to_string().bright_yellow().bold()
            );
        }
        ArchiveCommands::Export(a) => {
            let source = service.export(&a.snapshot, &a.output)?;
            println!(
                "  {}  rune {} → {}",
                "✓".green().bold(),
                source.display().to_string().cyan(),
                a.output.display().to_string().bright_yellow().bold()
            );
        }
        ArchiveCommands::Import(a) => {
            let metadata = service.import(&a.input).await?;
            println!(
                "  {}  rune sealed: {}",
                "✓".green().bold(),
                metadata.id.bright_yellow().bold()
            );
            if let Some(tag) = &metadata.tag {
                println!("    {}  {}", "tag     ".dimmed(), tag.bright_cyan());
            }
            println!("    {}  {}", "realm   ".dimmed(), metadata.hostname.cyan());
            println!("    {}  {}", "captured".dimmed(), metadata.timestamp.cyan());
        }
    }
    println!();
    Ok(())
}
