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
    /// Hliðskjálf — interactive overview from the high seat.
    #[command(visible_aliases = ["dashboard"])]
    AllEye(AllEyeArgs),
    /// Forge a fresh vault at ~/.odin (or --odin-dir).
    Init(InitArgs),
    /// Configure the Bifrost (GitHub) and view local config.
    Config(ConfigArgs),
    /// Capture this realm into a rune (snapshot) in the vault.
    Snapshot(SnapshotArgs),
    /// Bind this realm to the vault — preview by default, `--apply` to execute.
    Restore(RestoreArgs),
    /// Cross the Bifrost — commit and push runes to GitHub.
    #[command(visible_aliases = ["backup", "backup-online", "sync-online", "sync-global"])]
    Sync(SyncArgs),
    /// Renew Mjölnir — check for and install Odin updates.
    Update(UpdateArgs),
    /// Eir's gaze — diagnose broken paths, missing tools, conflicts.
    Doctor(DoctorArgs),
    /// Drift between this realm and the vault.
    Diff(DiffArgs),
    /// Carve PowerShell bootstrap and restore scripts.
    Export(ExportArgs),
    /// List bound bindings (listening ports).
    Ports(PortsArgs),
    /// Sever a binding by port or PID — release the realm.
    #[command(visible_aliases = ["kill"])]
    Freeport(FreeportArgs),
    /// Watch the host of warriors (htop-style).
    Ps(PsArgs),
    /// Timeline of runes etched in the vault.
    History(crate::commands::history::HistoryArgs),
    /// Wind the realm back to a previous rune.
    Rollback(crate::commands::rollback::RollbackArgs),
    /// Bound launcher profiles (batmode).
    Batmode(crate::commands::batmode::BatmodeArgs),
    /// Hugin & Munin patrol — watch the realm for drift.
    Watch(crate::commands::watch::WatchArgs),
    /// Plugins bound to Odin.
    Plugin(crate::commands::plugin::PluginArgs),
    /// Bundle runes into shareable tar.gz archives.
    Archive(crate::commands::archive::ArchiveArgs),
    /// Bind a realm in Asgard (`odin activate asgard` opens the TUI).
    Activate(ActivateArgs),
    /// Open the Asgard profile realm (shortcut for `activate asgard`).
    Asgard(AsgardArgs),
    /// Unbind the active realm (env stays in spawned warriors).
    Deactivate(DeactivateArgs),
    /// Forge, edit, list, export, and import realms in Asgard.
    Profile(ProfileArgs),
    /// Show the bound realm and recent bindings.
    Current(CurrentArgs),
    /// Munin watches the network — test connectivity to developer services.
    Net(NetArgs),
}

#[derive(Debug, Args)]
pub struct ActivateArgs {
    /// Realm name to bind. Use `asgard` (or omit in a TTY) to open the interactive selector.
    pub name: Option<String>,

    /// Skip the interactive TUI even when a TTY is present.
    #[arg(long)]
    pub non_interactive: bool,

    /// Emit JSON activation report instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DeactivateArgs {}

#[derive(Debug, Args)]
pub struct AsgardArgs {
    #[arg(long)]
    pub non_interactive: bool,

    #[arg(long, help = "Emit JSON activation report instead of human text.")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommands,
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommands {
    List(ProfileListArgs),
    Create(ProfileCreateArgs),
    Delete(ProfileDeleteArgs),
    Edit(ProfileEditArgs),
    Export(ProfileExportArgs),
    Import(ProfileImportArgs),
}

#[derive(Debug, Args)]
pub struct ProfileListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ProfileCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProfileDeleteArgs {
    pub name: String,
    #[arg(long, help = "Skip confirmation prompt.")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ProfileEditArgs {
    pub name: String,
}

#[derive(Debug, Args)]
pub struct ProfileExportArgs {
    pub name: String,
    #[arg(long, value_name = "PATH", help = "Output .tar.gz path.")]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ProfileImportArgs {
    pub path: PathBuf,
    #[arg(long, help = "Overwrite an existing profile of the same name.")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct CurrentArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct AllEyeArgs {}

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
pub struct FreeportArgs {
    #[arg(value_name = "PORT|PID", help = "Port number or process ID to release")]
    pub target: String,

    #[arg(long, help = "Force release the binding without confirmation")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct PsArgs {}

#[derive(Debug, Args)]
pub struct NetArgs {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,

    #[arg(
        long,
        default_value = "",
        help = "Comma-separated list of targets to test (e.g. github.com,npmjs.org)"
    )]
    pub target: String,
}
