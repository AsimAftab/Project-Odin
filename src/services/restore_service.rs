//! Full-control restore: plan → (confirm) → apply.
//!
//! [`build_plan`] classifies every package and section BEFORE anything runs
//! (pure, unit-tested); [`RestoreService::execute`] carries the plan out,
//! classifying each install attempt into a [`RestoreReport`] whose manual-
//! install list collects everything Odin couldn't do (unavailable in the
//! manager, manager missing, no install command, failed).

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::RestoreArgs;
use crate::integrations::process::CommandOutput;
use crate::integrations::{git_cli, powershell, process, vscode as vscode_integration};
use crate::models::config::RestoreConfig;
use crate::models::environment::EnvironmentSnapshot;
use crate::models::git::GitConfigSnapshot;
use crate::models::package::{InstalledPackage, PackageManager, PackageSnapshot};
use crate::models::restore::{
    manager_label, InstallOutcome, ManualItem, PackageResult, PlanAction, PlannedPackage,
    RestorePlan, RestoreReport, RestoreSection, SectionPlan, SectionResult,
};
use crate::models::vscode::VsCodeExtensionsSnapshot;
use crate::services::manager_bootstrap;
use crate::services::storage::SnapshotStore;

/// The four snapshot sections a restore consumes, wherever they came from
/// (vault root, local history, or a platform fetch).
pub struct RestoreInputs<'a> {
    pub packages: &'a PackageSnapshot,
    pub environment: &'a EnvironmentSnapshot,
    pub vscode: &'a VsCodeExtensionsSnapshot,
    pub git: &'a GitConfigSnapshot,
}

/// Fully-resolved restore controls after merging config, CLI flags, and (if
/// used) the interactive picker.
#[derive(Debug, Clone)]
pub struct RestoreOptions {
    /// Enabled sections.
    pub sections: Vec<RestoreSection>,
    /// Why each disabled section is off ("disabled by config", …).
    pub disabled: Vec<(RestoreSection, String)>,
    /// Effective manager alias list (feeds `source_enabled`).
    pub managers: Vec<String>,
    /// Lowercased package ids to exclude.
    pub exclude: Vec<String>,
    pub continue_on_error: bool,
    pub bootstrap_managers: bool,
    pub non_interactive: bool,
    /// Suppress decorative output (implied by --json).
    pub quiet: bool,
}

impl RestoreOptions {
    /// Merge config defaults with CLI flags. `--only` is an exact section set
    /// and intentionally re-enables sections disabled in config — that's the
    /// point of full control. Otherwise config gates apply, minus `--skip`.
    pub fn resolve(config: &RestoreConfig, args: &RestoreArgs) -> Self {
        let mut sections = Vec::new();
        let mut disabled = Vec::new();

        for section in RestoreSection::ALL {
            if !args.only.is_empty() {
                if args.only.contains(&section) {
                    sections.push(section);
                } else {
                    disabled.push((section, "not in --only".to_string()));
                }
                continue;
            }
            let config_on = match section {
                RestoreSection::Packages => true,
                RestoreSection::Extensions => config.restore_vscode_extensions,
                RestoreSection::Git => config.restore_git_config,
                RestoreSection::Env => config.restore_user_environment,
                RestoreSection::Path => config.restore_path,
            };
            if !config_on {
                disabled.push((section, "disabled by config".to_string()));
            } else if args.skip.contains(&section) {
                disabled.push((section, "skipped by --skip".to_string()));
            } else {
                sections.push(section);
            }
        }

        let managers = if args.managers.is_empty() {
            config.package_managers.clone()
        } else {
            args.managers.clone()
        };

        Self {
            sections,
            disabled,
            managers,
            exclude: args.exclude.iter().map(|e| e.to_lowercase()).collect(),
            continue_on_error: args.continue_on_error,
            bootstrap_managers: args.bootstrap_managers,
            non_interactive: args.non_interactive || args.json,
            quiet: args.json,
        }
    }

    /// Options straight from config with no CLI flags — used by `odin
    /// rollback`, which predates the flag surface.
    pub fn from_config(config: &RestoreConfig, continue_on_error: bool) -> Self {
        let mut sections = Vec::new();
        let mut disabled = Vec::new();
        for section in RestoreSection::ALL {
            let on = match section {
                RestoreSection::Packages => true,
                RestoreSection::Extensions => config.restore_vscode_extensions,
                RestoreSection::Git => config.restore_git_config,
                RestoreSection::Env => config.restore_user_environment,
                RestoreSection::Path => config.restore_path,
            };
            if on {
                sections.push(section);
            } else {
                disabled.push((section, "disabled by config".to_string()));
            }
        }
        Self {
            sections,
            disabled,
            managers: config.package_managers.clone(),
            exclude: Vec::new(),
            continue_on_error,
            bootstrap_managers: false,
            non_interactive: false,
            quiet: false,
        }
    }

    pub fn section_enabled(&self, section: RestoreSection) -> bool {
        self.sections.contains(&section)
    }

    pub fn disabled_reason(&self, section: RestoreSection) -> Option<&str> {
        self.disabled
            .iter()
            .find(|(s, _)| *s == section)
            .map(|(_, reason)| reason.as_str())
    }

    /// Replace the enabled section set (interactive picker result). Newly
    /// unchecked sections get the picker reason.
    pub fn set_sections(&mut self, picked: Vec<RestoreSection>) {
        self.disabled.retain(|(s, _)| !picked.contains(s));
        for section in RestoreSection::ALL {
            if !picked.contains(&section) && self.disabled_reason(section).is_none() {
                self.disabled
                    .push((section, "not selected in picker".to_string()));
            }
        }
        self.sections = picked;
    }
}

pub struct RestoreService {
    store: SnapshotStore,
}

impl RestoreService {
    pub fn new(store: SnapshotStore) -> Self {
        Self { store }
    }

    /// True if a local history directory exists for `snapshot_id`. Used by
    /// `odin restore <id>` to decide whether to restore from local history or
    /// fall back to fetching the snapshot from the Odin Platform.
    pub fn has_local_history(&self, snapshot_id: &str) -> bool {
        self.store.root().join("history").join(snapshot_id).exists()
    }

    /// Reads the four sections from the vault root (the last `odin snapshot`).
    pub async fn load_vault(
        &self,
    ) -> Result<(
        PackageSnapshot,
        EnvironmentSnapshot,
        VsCodeExtensionsSnapshot,
        GitConfigSnapshot,
    )> {
        Ok((
            self.store.read_packages().await?,
            self.store.read_environment().await?,
            self.store.read_vscode().await?,
            self.store.read_git().await?,
        ))
    }

    /// Reads the four sections from `~/.odin/history/<snapshot_id>`.
    pub async fn load_history(
        &self,
        snapshot_id: &str,
    ) -> Result<(
        PackageSnapshot,
        EnvironmentSnapshot,
        VsCodeExtensionsSnapshot,
        GitConfigSnapshot,
    )> {
        let history_root = self.store.root().join("history").join(snapshot_id);
        if !history_root.exists() {
            anyhow::bail!(
                "Historical snapshot files not found at {} — was this snapshot captured before per-id history was added? Run `odin snapshot` again to create a restorable snapshot.",
                history_root.display()
            );
        }
        let store = SnapshotStore::new(history_root);
        Ok((
            store
                .read_packages()
                .await
                .with_context(|| format!("reading packages for snapshot {}", snapshot_id))?,
            store
                .read_environment()
                .await
                .with_context(|| format!("reading environment for snapshot {}", snapshot_id))?,
            store.read_vscode().await.with_context(|| {
                format!("reading vscode extensions for snapshot {}", snapshot_id)
            })?,
            store
                .read_git()
                .await
                .with_context(|| format!("reading git config for snapshot {}", snapshot_id))?,
        ))
    }

    /// Probes live machine state and builds the classification plan. Pure
    /// classification happens in [`build_plan`]; this just gathers the probes.
    pub async fn plan(
        &self,
        inputs: &RestoreInputs<'_>,
        options: &RestoreOptions,
    ) -> Result<RestorePlan> {
        let current = crate::integrations::package_managers::list_packages().await?;
        let detected: Vec<String> = crate::integrations::package_managers::detect_managers()
            .await
            .into_iter()
            .filter(|m| m.installed)
            .map(|m| m.name)
            .collect();
        Ok(build_plan(options, inputs, &current, &detected))
    }

    /// Carries out the plan: manager bootstrap, package installs, and the
    /// non-package sections. Never returns `Err` for individual package
    /// failures — those land in the report; only environmental errors (e.g.
    /// a broken vault) propagate.
    pub async fn execute(
        &self,
        mut plan: RestorePlan,
        inputs: &RestoreInputs<'_>,
        options: &RestoreOptions,
        snapshot_id: Option<&str>,
    ) -> Result<RestoreReport> {
        let bootstrapped = manager_bootstrap::bootstrap_missing(&mut plan, options).await?;

        let packages = self.execute_packages(&plan, options).await?;

        let extensions = if options.section_enabled(RestoreSection::Extensions) {
            execute_extensions(inputs.vscode, options).await
        } else {
            SectionResult::default()
        };
        let git = if options.section_enabled(RestoreSection::Git) {
            execute_git(inputs.git, options).await
        } else {
            SectionResult::default()
        };
        let (environment, path) = execute_environment(inputs.environment, options).await;

        let manual = manual_items(&packages);

        Ok(RestoreReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            snapshot: snapshot_id.map(ToOwned::to_owned),
            applied: true,
            plan,
            packages,
            extensions,
            git,
            environment,
            path,
            manual,
            bootstrapped_managers: bootstrapped,
        })
    }

    async fn execute_packages(
        &self,
        plan: &RestorePlan,
        options: &RestoreOptions,
    ) -> Result<Vec<PackageResult>> {
        let mut results = Vec::with_capacity(plan.packages.len());
        let will_install = plan.count(&PlanAction::WillInstall);
        let bar = if options.quiet || will_install == 0 {
            ProgressBar::hidden()
        } else {
            let bar = ProgressBar::new(will_install as u64);
            bar.set_style(ProgressStyle::with_template(
                "  {spinner:.yellow} [{elapsed_precise}] [{bar:32.yellow/blue}] {pos}/{len} {msg}",
            )?);
            bar
        };

        let mut aborted = false;
        for planned in &plan.packages {
            let outcome = match &planned.action {
                PlanAction::WillInstall if aborted => InstallOutcome::Skipped {
                    reason: "aborted after earlier failure".to_string(),
                },
                PlanAction::WillInstall => {
                    bar.set_message(planned.id.clone());
                    let command = planned
                        .install_command
                        .as_deref()
                        .expect("WillInstall implies install_command");
                    if !options.quiet {
                        println!("  {}  {}", "→".bright_blue().bold(), command.dimmed());
                    }
                    let (program, args) = split_command(command);
                    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
                    let result = process::capture(&program, &arg_refs).await;
                    let outcome = classify_install_result(&planned.source, &result);
                    if !options.quiet {
                        print_outcome_line(planned, &outcome);
                    }
                    if matches!(outcome, InstallOutcome::Failed { .. })
                        && !options.continue_on_error
                    {
                        aborted = true;
                    }
                    bar.inc(1);
                    outcome
                }
                PlanAction::AlreadyInstalled => {
                    if !options.quiet {
                        println!("  {}  {}", "·".green(), planned.id.dimmed());
                    }
                    InstallOutcome::AlreadyInstalled
                }
                PlanAction::ExcludedByUser => InstallOutcome::Skipped {
                    reason: "excluded by --exclude".to_string(),
                },
                PlanAction::DisabledByConfig => InstallOutcome::Skipped {
                    reason: "manager or section disabled".to_string(),
                },
                PlanAction::NoInstallCommand => InstallOutcome::NoInstallCommand,
                PlanAction::ManagerMissing => InstallOutcome::ManagerMissing,
            };
            results.push(PackageResult {
                id: planned.id.clone(),
                name: planned.name.clone(),
                version: planned.version.clone(),
                source: planned.source.clone(),
                install_command: planned.install_command.clone(),
                outcome,
            });
        }
        bar.finish_and_clear();
        Ok(results)
    }
}

/// True if a package's source manager is enabled in the manager list.
/// Alias-aware (`choco` == `chocolatey`). `Manual`/`Unknown` packages can't be
/// attributed to a manager, so they're always allowed (they only install if
/// they carry a command anyway).
pub fn source_enabled(source: &PackageManager, managers: &[String]) -> bool {
    let aliases: &[&str] = match source {
        PackageManager::Winget => &["winget"],
        PackageManager::Chocolatey => &["chocolatey", "choco"],
        PackageManager::Scoop => &["scoop"],
        PackageManager::Npm => &["npm"],
        PackageManager::Pip => &["pip"],
        PackageManager::Cargo => &["cargo"],
        PackageManager::Pipx => &["pipx"],
        PackageManager::Pnpm => &["pnpm"],
        PackageManager::Yarn => &["yarn"],
        PackageManager::DotnetTool => &["dotnet", "dotnet-tool", "dotnettool"],
        PackageManager::Go => &["go"],
        PackageManager::Uv => &["uv"],
        PackageManager::Manual | PackageManager::Unknown => return true,
    };
    managers
        .iter()
        .any(|m| aliases.iter().any(|a| m.eq_ignore_ascii_case(a)))
}

/// Pure classification of every package + section. No I/O — live machine
/// state (`current`, `detected_managers`) is passed in.
pub fn build_plan(
    options: &RestoreOptions,
    inputs: &RestoreInputs<'_>,
    current: &PackageSnapshot,
    detected_managers: &[String],
) -> RestorePlan {
    let packages_enabled = options.section_enabled(RestoreSection::Packages);

    let planned: Vec<PlannedPackage> = inputs
        .packages
        .packages
        .iter()
        .map(|p| {
            let action = if !packages_enabled {
                PlanAction::DisabledByConfig
            } else if options.exclude.contains(&p.id.to_lowercase()) {
                PlanAction::ExcludedByUser
            } else if !source_enabled(&p.source, &options.managers) {
                PlanAction::DisabledByConfig
            } else if installed(p, &current.packages) {
                PlanAction::AlreadyInstalled
            } else if p.install_command.is_none() {
                PlanAction::NoInstallCommand
            } else if !manager_detected(&p.source, detected_managers) {
                PlanAction::ManagerMissing
            } else {
                PlanAction::WillInstall
            };
            PlannedPackage {
                id: p.id.clone(),
                name: p.name.clone(),
                version: p.version.clone(),
                source: p.source.clone(),
                install_command: p.install_command.clone(),
                action,
            }
        })
        .collect();

    let mut missing_managers: Vec<PackageManager> = Vec::new();
    for p in &planned {
        if p.action == PlanAction::ManagerMissing && !missing_managers.contains(&p.source) {
            missing_managers.push(p.source.clone());
        }
    }

    let will_install = planned
        .iter()
        .filter(|p| p.action == PlanAction::WillInstall)
        .count();
    let env_count = inputs
        .environment
        .user_variables
        .iter()
        .filter(|v| !v.name.eq_ignore_ascii_case("PATH"))
        .count();

    let sections = RestoreSection::ALL
        .into_iter()
        .map(|section| {
            let item_count = match section {
                RestoreSection::Packages => will_install,
                RestoreSection::Extensions => inputs.vscode.extensions.len(),
                RestoreSection::Git => inputs.git.entries.len(),
                RestoreSection::Env => env_count,
                RestoreSection::Path => inputs.environment.path_entries.len(),
            };
            SectionPlan {
                section,
                enabled: options.section_enabled(section),
                reason: options
                    .disabled_reason(section)
                    .unwrap_or_default()
                    .to_string(),
                item_count,
            }
        })
        .collect();

    RestorePlan {
        packages: planned,
        sections,
        missing_managers,
    }
}

/// Whether a package's manager is present on this machine. Manual/Unknown are
/// exempt — they can't be probed and only run if they carry a command.
fn manager_detected(source: &PackageManager, detected: &[String]) -> bool {
    if matches!(source, PackageManager::Manual | PackageManager::Unknown) {
        return true;
    }
    let label = manager_label(source);
    detected.iter().any(|d| d.eq_ignore_ascii_case(label))
}

/// winget's APPINSTALLER_CLI_ERROR_NO_APPLICATIONS_FOUND (0x8A15002B) as the
/// i32 that `ExitStatus::code()` reports on Windows.
pub const WINGET_NOT_FOUND_CODE: i32 = -1978335212;

/// Classify a completed (or failed-to-start) install attempt. The winget
/// not-found detection uses the exit code as the primary signal; the stdout
/// substring is a fallback for winget builds that exit 0 — note it is
/// locale-dependent (English winget only).
pub fn classify_install_result(
    source: &PackageManager,
    result: &Result<CommandOutput>,
) -> InstallOutcome {
    let is_winget = matches!(source, PackageManager::Winget);
    match result {
        // The process could not be started at all — manager binary missing.
        Err(_) => InstallOutcome::ManagerMissing,
        Ok(out) if out.code == 0 => {
            if is_winget && out.stdout.contains("No package found") {
                InstallOutcome::UnavailableInManager
            } else {
                InstallOutcome::Installed
            }
        }
        Ok(out)
            if is_winget
                && (out.code == WINGET_NOT_FOUND_CODE
                    || out.stdout.contains("No package found")) =>
        {
            InstallOutcome::UnavailableInManager
        }
        Ok(out) => InstallOutcome::Failed {
            code: out.code,
            detail: excerpt(
                if out.stderr.is_empty() {
                    &out.stdout
                } else {
                    &out.stderr
                },
                200,
            ),
        },
    }
}

/// Last non-empty line of `text`, truncated to `max` chars — enough to say
/// what went wrong without dumping installer output into the report.
fn excerpt(text: &str, max: usize) -> String {
    let line = text
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() <= max {
        line.to_string()
    } else {
        let truncated: String = line.chars().take(max).collect();
        format!("{truncated}…")
    }
}

/// Builds the MANUAL INSTALL REQUIRED list from execution results. Pure.
pub fn manual_items(results: &[PackageResult]) -> Vec<ManualItem> {
    results
        .iter()
        .filter_map(|r| {
            let (reason, hint) = match &r.outcome {
                InstallOutcome::UnavailableInManager => (
                    format!("not available via {}", manager_label(&r.source)),
                    Some(format!("winget search \"{}\"", r.name)),
                ),
                InstallOutcome::ManagerMissing => (
                    format!(
                        "package manager `{}` is not installed",
                        manager_label(&r.source)
                    ),
                    manager_bootstrap::install_hint(&r.source),
                ),
                InstallOutcome::NoInstallCommand => (
                    "no install command captured in the snapshot".to_string(),
                    Some(format!("winget search \"{}\"", r.name)),
                ),
                InstallOutcome::Failed { code, detail } => (
                    format!("install failed (exit {code}): {detail}"),
                    r.install_command.clone(),
                ),
                _ => return None,
            };
            Some(ManualItem {
                id: r.id.clone(),
                name: r.name.clone(),
                version: r.version.clone(),
                source: r.source.clone(),
                reason,
                hint,
            })
        })
        .collect()
}

/// One human-readable line per install attempt (suppressed under --json).
fn print_outcome_line(planned: &PlannedPackage, outcome: &InstallOutcome) {
    match outcome {
        InstallOutcome::UnavailableInManager => println!(
            "  {}  {} is not available via {}",
            "!".yellow().bold(),
            planned.id.cyan(),
            manager_label(&planned.source)
        ),
        InstallOutcome::ManagerMissing => println!(
            "  {}  {} — {} could not be run",
            "!".yellow().bold(),
            planned.id.cyan(),
            manager_label(&planned.source)
        ),
        InstallOutcome::Failed { code, detail } => eprintln!(
            "  {}  {} failed (exit {code}): {detail}",
            "✗".red().bold(),
            planned.id.cyan()
        ),
        _ => {}
    }
}

fn installed(package: &InstalledPackage, current: &[InstalledPackage]) -> bool {
    current.iter().any(|candidate| {
        candidate.source == package.source && candidate.id.eq_ignore_ascii_case(&package.id)
    })
}

fn split_command(command: &str) -> (String, Vec<String>) {
    let mut parts = command.split_whitespace();
    let program = parts.next().unwrap_or_default().to_string();
    (program, parts.map(ToOwned::to_owned).collect())
}

async fn execute_extensions(
    vscode: &VsCodeExtensionsSnapshot,
    options: &RestoreOptions,
) -> SectionResult {
    let mut result = SectionResult::default();
    let Some(code) = vscode_integration::executable() else {
        if !vscode.extensions.is_empty() {
            result.failed = vscode.extensions.len();
            result.attempted = vscode.extensions.len();
            result
                .errors
                .push("VS Code (code) not found on this machine".to_string());
            if !options.quiet {
                println!(
                    "  {}  VS Code not found — {} extension(s) skipped",
                    "!".yellow().bold(),
                    vscode.extensions.len()
                );
            }
        }
        return result;
    };
    for extension in &vscode.extensions {
        result.attempted += 1;
        if !options.quiet {
            println!(
                "  {}  code --install-extension {}",
                "→".bright_blue().bold(),
                extension.identifier.dimmed()
            );
        }
        match process::checked(&code, &["--install-extension", &extension.identifier]).await {
            Ok(_) => result.succeeded += 1,
            Err(e) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("{}: {e:#}", extension.identifier));
                if !options.quiet {
                    eprintln!("  {}  {}: {e:#}", "✗".red().bold(), extension.identifier);
                }
                if !options.continue_on_error {
                    break;
                }
            }
        }
    }
    result
}

async fn execute_git(git: &GitConfigSnapshot, options: &RestoreOptions) -> SectionResult {
    let mut result = SectionResult::default();
    let Some(git_bin) = git_cli::executable() else {
        if !git.entries.is_empty() {
            result.attempted = git.entries.len();
            result.failed = git.entries.len();
            result
                .errors
                .push("git not found on this machine".to_string());
            if !options.quiet {
                println!(
                    "  {}  git not found — {} config entrie(s) skipped",
                    "!".yellow().bold(),
                    git.entries.len()
                );
            }
        }
        return result;
    };
    for entry in &git.entries {
        result.attempted += 1;
        if !options.quiet {
            println!(
                "  {}  git config --global {} <value>",
                "→".bright_blue().bold(),
                entry.key.cyan()
            );
        }
        match process::checked(&git_bin, &["config", "--global", &entry.key, &entry.value]).await {
            Ok(_) => result.succeeded += 1,
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("{}: {e:#}", entry.key));
                if !options.quiet {
                    eprintln!("  {}  {}: {e:#}", "✗".red().bold(), entry.key);
                }
                if !options.continue_on_error {
                    break;
                }
            }
        }
    }
    result
}

async fn execute_environment(
    environment: &EnvironmentSnapshot,
    options: &RestoreOptions,
) -> (SectionResult, SectionResult) {
    let mut env_result = SectionResult::default();
    let mut path_result = SectionResult::default();

    if options.section_enabled(RestoreSection::Env) {
        for variable in &environment.user_variables {
            if variable.name.eq_ignore_ascii_case("PATH") {
                continue;
            }
            env_result.attempted += 1;
            match powershell::set_user_env_var(&variable.name, &variable.value).await {
                Ok(_) => env_result.succeeded += 1,
                Err(e) => {
                    env_result.failed += 1;
                    env_result.errors.push(format!("{}: {e:#}", variable.name));
                    if !options.continue_on_error {
                        break;
                    }
                }
            }
        }
        if env_result.succeeded > 0 && !options.quiet {
            println!(
                "  {}  carved {} rune(s) into the environment",
                "✓".green().bold(),
                env_result.succeeded.to_string().cyan().bold()
            );
        }
    }

    if options.section_enabled(RestoreSection::Path) {
        let path_value = environment
            .path_entries
            .iter()
            .map(|entry| entry.value.as_str())
            .collect::<Vec<_>>()
            .join(";");
        if !path_value.is_empty() {
            path_result.attempted = environment.path_entries.len();
            match powershell::set_user_env_var("Path", &path_value).await {
                Ok(_) => {
                    path_result.succeeded = environment.path_entries.len();
                    if !options.quiet {
                        println!(
                            "  {}  PATH bound — {} entries",
                            "✓".green().bold(),
                            environment.path_entries.len().to_string().cyan().bold()
                        );
                    }
                }
                Err(e) => {
                    path_result.failed = environment.path_entries.len();
                    path_result.errors.push(format!("PATH: {e:#}"));
                }
            }
        }
    }

    (env_result, path_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::environment::EnvironmentSnapshot;
    use crate::models::git::GitConfigSnapshot;
    use crate::models::vscode::VsCodeExtensionsSnapshot;

    fn managers(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn pkg(id: &str, source: PackageManager, install_command: Option<&str>) -> InstalledPackage {
        InstalledPackage {
            id: id.to_string(),
            name: id.to_string(),
            version: Some("1.0".to_string()),
            source,
            install_command: install_command.map(ToOwned::to_owned),
        }
    }

    fn empty_env() -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            user_variables: vec![],
            machine_variables: vec![],
            path_entries: vec![],
            powershell_profile: None,
            terminal_settings: None,
        }
    }

    fn options_with(managers_list: &[&str], exclude: &[&str]) -> RestoreOptions {
        RestoreOptions {
            sections: RestoreSection::ALL.to_vec(),
            disabled: vec![],
            managers: managers(managers_list),
            exclude: exclude.iter().map(|e| e.to_lowercase()).collect(),
            continue_on_error: false,
            bootstrap_managers: false,
            non_interactive: true,
            quiet: true,
        }
    }

    fn ok_output(code: i32, stdout: &str, stderr: &str) -> Result<CommandOutput> {
        Ok(CommandOutput {
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            code,
        })
    }

    // --- source_enabled (pre-existing behavior) ---

    #[test]
    fn enabled_when_manager_listed() {
        let m = managers(&["winget", "scoop"]);
        assert!(source_enabled(&PackageManager::Winget, &m));
        assert!(source_enabled(&PackageManager::Scoop, &m));
        assert!(!source_enabled(&PackageManager::Npm, &m));
    }

    #[test]
    fn choco_alias_matches_chocolatey() {
        assert!(source_enabled(
            &PackageManager::Chocolatey,
            &managers(&["choco"])
        ));
        assert!(source_enabled(
            &PackageManager::Chocolatey,
            &managers(&["chocolatey"])
        ));
        assert!(!source_enabled(
            &PackageManager::Chocolatey,
            &managers(&["scoop"])
        ));
    }

    #[test]
    fn manual_and_unknown_always_enabled() {
        let empty: Vec<String> = vec![];
        assert!(source_enabled(&PackageManager::Manual, &empty));
        assert!(source_enabled(&PackageManager::Unknown, &empty));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(source_enabled(
            &PackageManager::Winget,
            &managers(&["WinGet"])
        ));
    }

    // --- build_plan ---

    #[test]
    fn plan_classifies_every_bucket() {
        let snapshot = PackageSnapshot {
            packages: vec![
                pkg(
                    "fresh.tool",
                    PackageManager::Winget,
                    Some("winget install fresh.tool"),
                ),
                pkg(
                    "present.tool",
                    PackageManager::Winget,
                    Some("winget install present.tool"),
                ),
                pkg(
                    "excluded.tool",
                    PackageManager::Winget,
                    Some("winget install excluded.tool"),
                ),
                pkg(
                    "npm-tool",
                    PackageManager::Npm,
                    Some("npm install -g npm-tool"),
                ),
                pkg("no-command", PackageManager::Winget, None),
                pkg(
                    "scoop-tool",
                    PackageManager::Scoop,
                    Some("scoop install scoop-tool"),
                ),
            ],
        };
        let current = PackageSnapshot {
            packages: vec![pkg("present.tool", PackageManager::Winget, None)],
        };
        let env = empty_env();
        let vscode = VsCodeExtensionsSnapshot { extensions: vec![] };
        let git = GitConfigSnapshot { entries: vec![] };
        let inputs = RestoreInputs {
            packages: &snapshot,
            environment: &env,
            vscode: &vscode,
            git: &git,
        };
        // npm disabled by manager list; scoop enabled but not detected.
        let options = options_with(&["winget", "scoop"], &["EXCLUDED.tool"]);
        let plan = build_plan(&options, &inputs, &current, &managers(&["winget"]));

        let action_of = |id: &str| {
            plan.packages
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.action.clone())
                .unwrap()
        };
        assert_eq!(action_of("fresh.tool"), PlanAction::WillInstall);
        assert_eq!(action_of("present.tool"), PlanAction::AlreadyInstalled);
        assert_eq!(action_of("excluded.tool"), PlanAction::ExcludedByUser);
        assert_eq!(action_of("npm-tool"), PlanAction::DisabledByConfig);
        assert_eq!(action_of("no-command"), PlanAction::NoInstallCommand);
        assert_eq!(action_of("scoop-tool"), PlanAction::ManagerMissing);
        assert_eq!(plan.missing_managers, vec![PackageManager::Scoop]);
        assert_eq!(plan.count(&PlanAction::WillInstall), 1);
    }

    #[test]
    fn plan_disables_all_packages_when_section_off() {
        let snapshot = PackageSnapshot {
            packages: vec![pkg("x", PackageManager::Winget, Some("winget install x"))],
        };
        let current = PackageSnapshot { packages: vec![] };
        let env = empty_env();
        let vscode = VsCodeExtensionsSnapshot { extensions: vec![] };
        let git = GitConfigSnapshot { entries: vec![] };
        let inputs = RestoreInputs {
            packages: &snapshot,
            environment: &env,
            vscode: &vscode,
            git: &git,
        };
        let mut options = options_with(&["winget"], &[]);
        options.set_sections(vec![RestoreSection::Git]);
        let plan = build_plan(&options, &inputs, &current, &managers(&["winget"]));
        assert_eq!(plan.packages[0].action, PlanAction::DisabledByConfig);
        assert!(plan.missing_managers.is_empty());
    }

    // --- classify_install_result ---

    #[test]
    fn classify_success() {
        let r = ok_output(0, "installed", "");
        assert_eq!(
            classify_install_result(&PackageManager::Winget, &r),
            InstallOutcome::Installed
        );
    }

    #[test]
    fn classify_winget_not_found_by_code() {
        let r = ok_output(WINGET_NOT_FOUND_CODE, "", "");
        assert_eq!(
            classify_install_result(&PackageManager::Winget, &r),
            InstallOutcome::UnavailableInManager
        );
    }

    #[test]
    fn classify_winget_not_found_by_stdout_even_on_zero_exit() {
        let r = ok_output(0, "No package found matching input criteria.", "");
        assert_eq!(
            classify_install_result(&PackageManager::Winget, &r),
            InstallOutcome::UnavailableInManager
        );
    }

    #[test]
    fn classify_generic_failure_carries_detail() {
        let r = ok_output(1, "", "npm ERR! network timeout");
        match classify_install_result(&PackageManager::Npm, &r) {
            InstallOutcome::Failed { code, detail } => {
                assert_eq!(code, 1);
                assert!(detail.contains("network timeout"));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn classify_process_start_error_as_manager_missing() {
        let r: Result<CommandOutput> = Err(anyhow::anyhow!("program not found"));
        assert_eq!(
            classify_install_result(&PackageManager::Scoop, &r),
            InstallOutcome::ManagerMissing
        );
    }

    // --- RestoreOptions::resolve is exercised via cli parsing in command
    //     tests; core rules covered here through set_sections + section flags.

    #[test]
    fn manual_items_collects_attention_outcomes() {
        let results = vec![
            PackageResult {
                id: "a".into(),
                name: "a".into(),
                version: None,
                source: PackageManager::Winget,
                install_command: Some("winget install a".into()),
                outcome: InstallOutcome::UnavailableInManager,
            },
            PackageResult {
                id: "b".into(),
                name: "b".into(),
                version: None,
                source: PackageManager::Scoop,
                install_command: Some("scoop install b".into()),
                outcome: InstallOutcome::ManagerMissing,
            },
            PackageResult {
                id: "c".into(),
                name: "c".into(),
                version: None,
                source: PackageManager::Winget,
                install_command: None,
                outcome: InstallOutcome::NoInstallCommand,
            },
            PackageResult {
                id: "d".into(),
                name: "d".into(),
                version: None,
                source: PackageManager::Npm,
                install_command: Some("npm install -g d".into()),
                outcome: InstallOutcome::Failed {
                    code: 1,
                    detail: "boom".into(),
                },
            },
            PackageResult {
                id: "ok".into(),
                name: "ok".into(),
                version: None,
                source: PackageManager::Winget,
                install_command: Some("winget install ok".into()),
                outcome: InstallOutcome::Installed,
            },
        ];
        let manual = manual_items(&results);
        let ids: Vec<&str> = manual.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
        assert!(manual[0].hint.as_deref().unwrap().contains("winget search"));
        assert!(manual[1].reason.contains("scoop"));
    }

    #[test]
    fn excerpt_truncates_and_takes_last_line() {
        assert_eq!(excerpt("line1\nline2\n", 100), "line2");
        let long = "x".repeat(300);
        assert!(excerpt(&long, 200).ends_with('…'));
    }

    #[test]
    fn report_serializes_with_tagged_outcomes() {
        let report = RestoreReport::dry_run(
            Some("snap-1"),
            RestorePlan {
                packages: vec![PlannedPackage {
                    id: "x".into(),
                    name: "x".into(),
                    version: None,
                    source: PackageManager::Winget,
                    install_command: None,
                    action: PlanAction::NoInstallCommand,
                }],
                sections: vec![],
                missing_managers: vec![],
            },
        );
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["applied"], false);
        assert_eq!(value["snapshot"], "snap-1");
        assert_eq!(value["plan"]["packages"][0]["action"], "no_install_command");
    }
}
