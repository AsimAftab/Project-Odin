//! Plan/report types for the full-control restore flow (`odin restore`).
//!
//! Restore is plan → (confirm) → apply: a [`RestorePlan`] classifies every
//! package and section BEFORE anything runs, and a [`RestoreReport`] records
//! what actually happened — including the "manual install required" list for
//! everything Odin couldn't handle. The report is written to
//! `~/.odin/logs/restore-<ts>.json` and printed with `--json`.

use serde::{Deserialize, Serialize};

use crate::models::package::PackageManager;

/// Sections a restore can touch. Doubles as the `--only`/`--skip` value enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreSection {
    Packages,
    Extensions,
    Git,
    Env,
    Path,
    Terminal,
    PsProfile,
}

impl RestoreSection {
    pub const ALL: [RestoreSection; 7] = [
        RestoreSection::Packages,
        RestoreSection::Extensions,
        RestoreSection::Git,
        RestoreSection::Env,
        RestoreSection::Path,
        RestoreSection::Terminal,
        RestoreSection::PsProfile,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            RestoreSection::Packages => "packages",
            RestoreSection::Extensions => "extensions",
            RestoreSection::Git => "git",
            RestoreSection::Env => "env",
            RestoreSection::Path => "path",
            RestoreSection::Terminal => "terminal",
            RestoreSection::PsProfile => "ps-profile",
        }
    }
}

/// Why a package will or won't be installed — decided before touching anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PlanAction {
    WillInstall,
    AlreadyInstalled,
    /// Manager not in the effective manager list, or the whole packages
    /// section is disabled.
    DisabledByConfig,
    /// Matched `--exclude`.
    ExcludedByUser,
    NoInstallCommand,
    /// Manager is enabled but not detected on this machine.
    ManagerMissing,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedPackage {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub source: PackageManager,
    pub install_command: Option<String>,
    #[serde(flatten)]
    pub action: PlanAction,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionPlan {
    pub section: RestoreSection,
    pub enabled: bool,
    /// Why it's disabled ("disabled by config", "skipped by --skip", …) or "".
    pub reason: String,
    /// Items the section would touch (WillInstall count for packages).
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestorePlan {
    pub packages: Vec<PlannedPackage>,
    pub sections: Vec<SectionPlan>,
    /// Managers with WillInstall-worthy packages that aren't installed here —
    /// bootstrap candidates.
    pub missing_managers: Vec<PackageManager>,
}

impl RestorePlan {
    pub fn count(&self, action: &PlanAction) -> usize {
        self.packages.iter().filter(|p| &p.action == action).count()
    }
}

/// Outcome of one package during `--apply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum InstallOutcome {
    Installed,
    AlreadyInstalled,
    Skipped {
        reason: String,
    },
    /// The manager says the package doesn't exist (e.g. winget
    /// "No package found matching input criteria").
    UnavailableInManager,
    ManagerMissing,
    NoInstallCommand,
    Failed {
        code: i32,
        detail: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageResult {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub source: PackageManager,
    pub install_command: Option<String>,
    #[serde(flatten)]
    pub outcome: InstallOutcome,
}

/// Aggregate result for a non-package section (extensions/git/env/path).
#[derive(Debug, Clone, Default, Serialize)]
pub struct SectionResult {
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// One line of the MANUAL INSTALL REQUIRED list.
#[derive(Debug, Clone, Serialize)]
pub struct ManualItem {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub source: PackageManager,
    pub reason: String,
    pub hint: Option<String>,
}

/// Full restore report. `applied == false` means dry-run: `packages` results
/// are empty and only the plan is meaningful.
#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    pub timestamp: String,
    pub snapshot: Option<String>,
    pub applied: bool,
    pub plan: RestorePlan,
    pub packages: Vec<PackageResult>,
    pub extensions: SectionResult,
    pub git: SectionResult,
    pub environment: SectionResult,
    pub path: SectionResult,
    pub terminal: SectionResult,
    pub ps_profile: SectionResult,
    pub manual: Vec<ManualItem>,
    pub bootstrapped_managers: Vec<String>,
}

impl RestoreReport {
    pub fn dry_run(snapshot: Option<&str>, plan: RestorePlan) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            snapshot: snapshot.map(ToOwned::to_owned),
            applied: false,
            plan,
            packages: Vec::new(),
            extensions: SectionResult::default(),
            git: SectionResult::default(),
            environment: SectionResult::default(),
            path: SectionResult::default(),
            terminal: SectionResult::default(),
            ps_profile: SectionResult::default(),
            manual: Vec::new(),
            bootstrapped_managers: Vec::new(),
        }
    }

    /// True when anything actually errored (drives the non-zero exit code).
    /// Manual-list-only outcomes (unavailable / no command / manager missing)
    /// are "needs attention", not failures.
    pub fn has_failures(&self) -> bool {
        self.packages
            .iter()
            .any(|p| matches!(p.outcome, InstallOutcome::Failed { .. }))
            || self.extensions.failed > 0
            || self.git.failed > 0
            || self.environment.failed > 0
            || self.path.failed > 0
            || self.terminal.failed > 0
            || self.ps_profile.failed > 0
    }

    pub fn installed_count(&self) -> usize {
        self.packages
            .iter()
            .filter(|p| p.outcome == InstallOutcome::Installed)
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.packages
            .iter()
            .filter(|p| matches!(p.outcome, InstallOutcome::Failed { .. }))
            .count()
    }
}

/// Canonical manager name as used by `detect_managers()` probes and the
/// bootstrap recipes.
pub fn manager_label(manager: &PackageManager) -> &'static str {
    match manager {
        PackageManager::Winget => "winget",
        PackageManager::Chocolatey => "choco",
        PackageManager::Scoop => "scoop",
        PackageManager::Npm => "npm",
        PackageManager::Pip => "pip",
        PackageManager::Cargo => "cargo",
        PackageManager::Pipx => "pipx",
        PackageManager::Pnpm => "pnpm",
        PackageManager::Yarn => "yarn",
        PackageManager::DotnetTool => "dotnet",
        PackageManager::Go => "go",
        PackageManager::Uv => "uv",
        PackageManager::Manual => "manual",
        PackageManager::Unknown => "unknown",
    }
}
