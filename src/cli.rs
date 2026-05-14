use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "odin",
    version,
    about = "Developer workstation snapshot and restore manager"
)]
pub struct Cli {
    #[arg(long, env = "ODIN_DIR", global = true)]
    pub odin_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Dashboard(DashboardArgs),
    Init(InitArgs),
    Config(ConfigArgs),
    Snapshot(SnapshotArgs),
    Restore(RestoreArgs),
    #[command(visible_aliases = ["backup", "backup-online", "sync-online", "sync-global"])]
    Sync(SyncArgs),
    Update(UpdateArgs),
    Doctor(DoctorArgs),
    Diff(DiffArgs),
    Export(ExportArgs),
    Ports(PortsArgs),
    Kill(KillArgs),
    Ps(PsArgs),
    History(crate::commands::history::HistoryArgs),
    Rollback(crate::commands::rollback::RollbackArgs),
    Batmode(crate::commands::batmode::BatmodeArgs),
    Watch(crate::commands::watch::WatchArgs),
    Plugin(crate::commands::plugin::PluginArgs),
    Archive(crate::commands::archive::ArchiveArgs),
}

#[derive(Debug, Args)]
pub struct DashboardArgs {}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    Github(ConfigGithubArgs),
    Show(ConfigShowArgs),
}

#[derive(Debug, Args)]
pub struct ConfigGithubArgs {
    #[arg(long)]
    pub repo: Option<String>,

    #[arg(long, default_value = "main")]
    pub branch: String,

    #[arg(long, env = "GITHUB_TOKEN")]
    pub token: Option<String>,

    #[arg(long)]
    pub non_interactive: bool,

    #[arg(
        long,
        help = "Immediately commit and push current Odin state after saving GitHub config."
    )]
    pub sync_now: bool,
}

#[derive(Debug, Args)]
pub struct ConfigShowArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotArgs {
    #[arg(long)]
    pub include_machine_env: bool,

    /// Optional human-readable tag for this snapshot (e.g. "prod", "before-migration").
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    #[arg(
        long,
        help = "Execute restore commands. Without this flag Odin prints a dry run."
    )]
    pub apply: bool,

    #[arg(long, help = "Continue restoring after a package command fails.")]
    pub continue_on_error: bool,
}

#[derive(Debug, Args)]
pub struct SyncArgs {
    #[arg(long)]
    pub remote: Option<String>,

    #[arg(
        long,
        help = "Create a GitHub repository before pushing, using GITHUB_TOKEN or --github-token."
    )]
    pub create_private_repo: bool,

    #[arg(
        long,
        value_name = "NAME",
        help = "Repository name to create when --create-private-repo is used."
    )]
    pub github_repo: Option<String>,

    #[arg(long, env = "GITHUB_TOKEN")]
    pub github_token: Option<String>,

    #[arg(long, default_value = "main")]
    pub branch: String,

    #[arg(long)]
    pub message: Option<String>,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    #[arg(
        long,
        help = "Only check if an update is available without installing it."
    )]
    pub check: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DiffArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct PortsArgs {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct KillArgs {
    #[arg(value_name = "PORT|PID", help = "Port number or process ID to kill")]
    pub target: String,

    #[arg(long, help = "Force kill process without confirmation")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct PsArgs {}
