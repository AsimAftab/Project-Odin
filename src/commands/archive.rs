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
    println!("{}", "Archive".bold().cyan());
    println!("{}\n", rule(60));
    match args.command {
        ArchiveCommands::Create(a) => {
            service.create(&a.input_dir, &a.output)?;
            println!(
                "{} archived {} -> {}",
                "ok".green(),
                a.input_dir.display(),
                a.output.display()
            );
        }
        ArchiveCommands::Extract(a) => {
            service.extract(&a.input, &a.output_dir)?;
            println!(
                "{} extracted {} -> {}",
                "ok".green(),
                a.input.display(),
                a.output_dir.display()
            );
        }
        ArchiveCommands::Export(a) => {
            let source = service.export(&a.snapshot, &a.output)?;
            println!(
                "{} exported snapshot {} -> {}",
                "ok".green(),
                source.display(),
                a.output.display()
            );
        }
        ArchiveCommands::Import(a) => {
            let metadata = service.import(&a.input).await?;
            println!(
                "{} imported snapshot {}",
                "ok".green(),
                metadata.id.bright_yellow()
            );
            if let Some(tag) = &metadata.tag {
                println!("    tag: {}", tag.bright_cyan());
            }
            println!("    hostname: {}", metadata.hostname);
            println!("    captured: {}", metadata.timestamp);
        }
    }
    Ok(())
}
