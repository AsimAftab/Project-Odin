use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use colored::Colorize;
use dialoguer::{Confirm, FuzzySelect, Input, Select};
use indicatif::{ProgressBar, ProgressStyle};

use crate::asgard::profile::{
    validate_name, BrowserEntry, LayoutPreset, Profile, StartupApp, WindowLayout, WindowState,
};
use crate::asgard::store::AsgardStore;
use crate::integrations::launcher;

/// Outcome of activating a profile, used for printing summaries and JSON output.
#[derive(Debug, Default)]
pub struct ActivationReport {
    pub profile: String,
    pub started: Vec<String>,
    pub failed: Vec<(String, String)>,
}

pub async fn activate(odin_dir: &Path, name: &str) -> Result<ActivationReport> {
    let store = AsgardStore::new(odin_dir);
    let profile = store
        .load(name)
        .await
        .map_err(|e| anyhow!("{e}. Run `odin activate asgard` to create one."))?;

    let mut state = store.load_state().await?;
    let now = Utc::now();
    state.record_activation(&profile.name, now);
    store.save_state(&state).await?;

    let mut report = ActivationReport {
        profile: profile.name.clone(),
        ..Default::default()
    };

    if let Some(ws) = &profile.vscode_workspace {
        let label = format!("vscode: {ws}");
        match launcher::open_vscode_workspace(Path::new(ws)) {
            Ok(()) => report.started.push(label),
            Err(e) => report.failed.push((label, first_line(&e.to_string()))),
        }
    }

    for app in &profile.startup_apps {
        match launcher::launch(app, &profile.env) {
            Ok(()) => report.started.push(format!("app: {}", app.name)),
            Err(e) => report
                .failed
                .push((format!("app: {}", app.name), first_line(&e.to_string()))),
        }
    }

    for entry in &profile.browser_urls {
        let label = format!("url: {} ({})", entry.name, entry.url);
        match launcher::launch_url(&entry.url) {
            Ok(()) => report.started.push(label),
            Err(e) => report.failed.push((label, first_line(&e.to_string()))),
        }
    }

    let layouts_to_apply: Vec<_> = profile
        .startup_apps
        .iter()
        .filter(|app| app.layout.is_some())
        .map(|app| {
            (
                app.name.clone(),
                app.command.clone(),
                app.layout.clone().unwrap(),
            )
        })
        .collect();

    if !layouts_to_apply.is_empty() {
        let spinner = ProgressBar::new_spinner();
        if let Ok(style) = ProgressStyle::with_template("  {spinner:.yellow} {msg}") {
            spinner.set_style(style);
        }
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        spinner.set_message("applying window layouts (waiting for apps to start)...");

        for _ in 0..15 {
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
            let mut all_done = true;
            for (app_name, command, layout) in &layouts_to_apply {
                let exe_name = std::path::Path::new(command)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(command);

                // Try exe name
                if crate::integrations::window_manager::apply_layout(exe_name, layout).is_err() {
                    // Fallback to app name (like "Calculator")
                    if crate::integrations::window_manager::apply_layout(app_name, layout).is_err()
                    {
                        all_done = false;
                    }
                }
            }
            if all_done {
                break;
            }
        }
        spinner.finish_and_clear();
    }

    Ok(report)
}

pub async fn deactivate(odin_dir: &Path) -> Result<Option<String>> {
    let store = AsgardStore::new(odin_dir);
    let mut state = store.load_state().await?;
    let prev = state.active_profile.clone();
    state.clear_active();
    store.save_state(&state).await?;
    Ok(prev)
}

pub async fn delete(odin_dir: &Path, name: &str) -> Result<()> {
    let store = AsgardStore::new(odin_dir);
    if !store.exists(name) {
        bail!("profile `{name}` not found");
    }
    store.delete(name).await?;
    let mut state = store.load_state().await?;
    state.drop_profile(name);
    store.save_state(&state).await?;
    Ok(())
}

pub fn print_activation_report(report: &ActivationReport) {
    let rule: String = "─".repeat(54);
    println!();
    println!(
        "  {}  realm {} bound",
        "✓".green().bold(),
        report.profile.bright_yellow().bold()
    );
    println!("  {}", rule.dimmed());

    if report.started.is_empty() && report.failed.is_empty() {
        println!(
            "  {}  the realm is bound, but quiet — no warriors, ravens, or workspace declared",
            "·".dimmed()
        );
        println!();
        return;
    }

    // Group started entries by their prefix (app:, url:, vscode:) for cleaner output.
    let mut warriors: Vec<&str> = Vec::new();
    let mut ravens: Vec<&str> = Vec::new();
    let mut workspaces: Vec<&str> = Vec::new();
    for s in &report.started {
        if let Some(rest) = s.strip_prefix("app: ") {
            warriors.push(rest);
        } else if let Some(rest) = s.strip_prefix("url: ") {
            ravens.push(rest);
        } else if let Some(rest) = s.strip_prefix("vscode: ") {
            workspaces.push(rest);
        } else {
            warriors.push(s.as_str());
        }
    }

    if !warriors.is_empty() {
        println!(
            "  {}  ⚒ warriors  ({})",
            "·".dimmed(),
            warriors.len().to_string().bright_blue().bold()
        );
        for w in &warriors {
            println!("      {} {}", "→".green(), w);
        }
    }
    if !ravens.is_empty() {
        println!(
            "  {}  ⌒ ravens    ({})",
            "·".dimmed(),
            ravens.len().to_string().bright_blue().bold()
        );
        for r in &ravens {
            println!("      {} {}", "→".green(), r);
        }
    }
    if !workspaces.is_empty() {
        println!(
            "  {}  ◇ vscode    ({})",
            "·".dimmed(),
            workspaces.len().to_string().bright_blue().bold()
        );
        for w in &workspaces {
            println!("      {} {}", "→".green(), w);
        }
    }
    if !report.failed.is_empty() {
        println!();
        println!(
            "  {}  shattered    ({})",
            "✗".red().bold(),
            report.failed.len().to_string().red().bold()
        );
        for (label, err) in &report.failed {
            println!("      {} {} — {}", "✗".red(), label, err.dimmed());
        }
    }
    println!();
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

/// Interactive wizard. Runs outside the ratatui alt screen.
pub async fn wizard(odin_dir: &Path, suggested_name: Option<String>) -> Result<Profile> {
    let store = AsgardStore::new(odin_dir);
    let existing = store.list().await.unwrap_or_default();

    print_wizard_banner();

    let name = match suggested_name {
        Some(n) => {
            validate_name(&n).map_err(|e| anyhow!(e))?;
            if existing.iter().any(|x| x == &n) {
                bail!("realm `{n}` already exists in Asgard");
            }
            n
        }
        None => prompt_name(&existing)?,
    };

    let description: String = Input::new()
        .with_prompt("Description (optional)")
        .allow_empty(true)
        .interact_text()?;

    print_section("ᚱ", "Runes — environment variables");
    let env = prompt_env_vars()?;
    print_section("⚒", "Warriors — startup apps");
    let startup_apps = prompt_startup_apps().await?;
    print_section("◇", "Forge — VS Code workspace");
    let vscode_workspace = prompt_optional_path("VS Code workspace path (blank to skip)")?;
    print_section("⌒", "Ravens — browser URLs");
    let browser_urls = prompt_browser_urls()?;

    let profile = Profile {
        name,
        description,
        env,
        startup_apps,
        vscode_workspace,
        browser_urls,
    };

    store.save(&profile).await?;
    print_forged_card(&profile, &store.profile_path(&profile.name));
    Ok(profile)
}

fn print_forged_card(profile: &Profile, path: &Path) {
    let rule: String = "─".repeat(58);
    println!();
    println!(
        "  {}  {}",
        "✓".green().bold(),
        format!("realm {} forged in Asgard", profile.name)
            .bright_white()
            .bold()
    );
    println!("  {}", rule.dimmed());
    if !profile.description.is_empty() {
        println!("  {}  {}", "·".dimmed(), profile.description.italic());
    }
    println!(
        "  {}  ᚱ runes      {}",
        "·".dimmed(),
        profile.env.len().to_string().bright_blue().bold()
    );
    println!(
        "  {}  ⚒ warriors   {}",
        "·".dimmed(),
        profile.startup_apps.len().to_string().bright_blue().bold()
    );
    println!(
        "  {}  ⌒ ravens     {}",
        "·".dimmed(),
        profile.browser_urls.len().to_string().bright_blue().bold()
    );
    if profile.vscode_workspace.is_some() {
        println!("  {}  ◇ vscode     {}", "·".dimmed(), "linked".green());
    }
    println!("  {}", rule.dimmed());
    println!(
        "  {}  scribed at {}",
        "→".bright_blue(),
        path.display().to_string().dimmed()
    );
    println!();
}

fn print_wizard_banner() {
    println!();
    println!(
        "  {}  {}",
        "ᚨ".bright_yellow().bold(),
        "ASGARD — forge a new realm".bright_white().bold()
    );
    println!("  {}", "─".repeat(54).dimmed());
}

fn print_section(glyph: &str, title: &str) {
    println!();
    println!(
        "  {}  {}",
        glyph.bright_yellow().bold(),
        title.bright_white().bold()
    );
}

fn prompt_name(existing: &[String]) -> Result<String> {
    loop {
        let raw: String = Input::new().with_prompt("Realm name").interact_text()?;
        match validate_name(&raw) {
            Ok(()) if existing.iter().any(|x| x == &raw) => {
                println!(
                    "  {} `{raw}` is already a realm — choose another rune",
                    "!".yellow()
                );
            }
            Ok(()) => return Ok(raw),
            Err(e) => println!("  {} {e}", "!".yellow()),
        }
    }
}

fn prompt_env_vars() -> Result<BTreeMap<String, String>> {
    println!(
        "  {}",
        "carve KEY=VALUE one at a time — blank line to finish".dimmed()
    );
    let mut env = BTreeMap::new();
    loop {
        let raw: String = Input::new()
            .with_prompt("  KEY=VALUE")
            .allow_empty(true)
            .interact_text()?;
        if raw.trim().is_empty() {
            break;
        }
        match raw.split_once('=') {
            Some((k, v)) if !k.trim().is_empty() => {
                env.insert(k.trim().to_string(), v.to_string());
            }
            _ => println!("    {} expected `KEY=VALUE`", "!".yellow()),
        }
    }
    Ok(env)
}

async fn prompt_startup_apps() -> Result<Vec<StartupApp>> {
    println!(
        "  {}",
        "summon the warriors that ride out at activation".dimmed()
    );
    let mut apps = Vec::new();
    while let Some(app) = prompt_one_startup_app().await? {
        apps.push(app);
    }
    Ok(apps)
}

async fn prompt_one_startup_app() -> Result<Option<StartupApp>> {
    let sources = [
        "Pick from installed warriors",
        "Carve a command (exe, .lnk, URL)",
        "Cancel",
    ];
    let pick = Select::new()
        .with_prompt("  Where does this warrior come from?")
        .items(&sources)
        .default(0)
        .interact()?;
    match pick {
        0 => prompt_via_installed_picker().await,
        1 => prompt_via_manual_command(),
        _ => Ok(None),
    }
}

async fn prompt_via_installed_picker() -> Result<Option<StartupApp>> {
    let apps = match cached_installed_apps().await {
        Ok(list) if !list.is_empty() => list,
        Ok(_) => {
            println!(
                "  {} no installed apps found via Get-StartApps; falling back to manual entry",
                "!".yellow()
            );
            return prompt_via_manual_command();
        }
        Err(e) => {
            println!(
                "  {} couldn't list installed apps ({}); falling back to manual entry",
                "!".yellow(),
                first_line(&e.to_string()).dimmed()
            );
            return prompt_via_manual_command();
        }
    };

    let display: Vec<String> = apps.iter().map(|a| a.name.clone()).collect();
    let pick = FuzzySelect::new()
        .with_prompt("  Search apps")
        .items(&display)
        .default(0)
        .interact_opt()?;
    let Some(idx) = pick else {
        return Ok(None);
    };
    let chosen = &apps[idx];

    let window = prompt_window_state()?;
    let layout = prompt_window_layout()?;

    Ok(Some(StartupApp {
        name: chosen.name.clone(),
        command: format!("shell:AppsFolder\\{}", chosen.app_id),
        args: Vec::new(),
        cwd: None,
        window,
        layout,
    }))
}

fn prompt_via_manual_command() -> Result<Option<StartupApp>> {
    let cmd: String = Input::new()
        .with_prompt("  command")
        .allow_empty(true)
        .interact_text()?;
    if cmd.trim().is_empty() {
        return Ok(None);
    }
    let default_label = guess_label(&cmd);
    let name: String = Input::new()
        .with_prompt("  label")
        .default(default_label)
        .interact_text()?;
    let raw_args: String = Input::new()
        .with_prompt("  args (space-separated, blank for none)")
        .allow_empty(true)
        .interact_text()?;
    let args: Vec<String> = shell_split(&raw_args);
    let cwd = prompt_optional_path("  cwd (blank for none)")?;
    let window = prompt_window_state()?;
    let layout = prompt_window_layout()?;
    Ok(Some(StartupApp {
        name,
        command: cmd,
        args,
        cwd,
        window,
        layout,
    }))
}

fn prompt_window_state() -> Result<WindowState> {
    let window_choices = ["normal", "minimized", "maximized"];
    let pick = Select::new()
        .with_prompt("  window state")
        .items(&window_choices)
        .default(0)
        .interact()?;
    Ok(match pick {
        1 => WindowState::Minimized,
        2 => WindowState::Maximized,
        _ => WindowState::Normal,
    })
}

fn prompt_window_layout() -> Result<Option<WindowLayout>> {
    let layout_choices = [
        "None (let OS decide)",
        "Snap Left",
        "Snap Right",
        "Top Half",
        "Bottom Half",
        "Quadrant 1 (Top Right)",
        "Quadrant 2 (Top Left)",
        "Quadrant 3 (Bottom Left)",
        "Quadrant 4 (Bottom Right)",
    ];
    let pick = Select::new()
        .with_prompt("  window layout (snap position)")
        .items(&layout_choices)
        .default(0)
        .interact()?;

    let preset = match pick {
        1 => Some(LayoutPreset::SnapLeft),
        2 => Some(LayoutPreset::SnapRight),
        3 => Some(LayoutPreset::TopHalf),
        4 => Some(LayoutPreset::BottomHalf),
        5 => Some(LayoutPreset::Quadrant1),
        6 => Some(LayoutPreset::Quadrant2),
        7 => Some(LayoutPreset::Quadrant3),
        8 => Some(LayoutPreset::Quadrant4),
        _ => None,
    };

    let Some(preset) = preset else {
        return Ok(None);
    };

    let monitor = prompt_layout_monitor()?;
    if monitor <= 1 {
        Ok(Some(WindowLayout::Preset(preset)))
    } else {
        Ok(Some(WindowLayout::TargetedPreset { preset, monitor }))
    }
}

fn prompt_layout_monitor() -> Result<u32> {
    let monitors = match crate::integrations::window_manager::list_display_monitors() {
        Ok(monitors) => monitors,
        Err(e) => {
            println!(
                "  {} couldn't detect monitors ({}); using primary monitor",
                "!".yellow(),
                first_line(&e.to_string()).dimmed()
            );
            return Ok(1);
        }
    };

    if monitors.len() <= 1 {
        return Ok(1);
    }

    let choices: Vec<String> = monitors
        .iter()
        .map(|monitor| {
            let primary = if monitor.is_primary { " primary" } else { "" };
            let device = if monitor.device_name.is_empty() {
                String::new()
            } else {
                format!(" {}", monitor.device_name)
            };
            format!(
                "Display {}{} - {}{} ({}x{} at {},{})",
                monitor.index,
                primary,
                monitor.name,
                device,
                monitor.width,
                monitor.height,
                monitor.left,
                monitor.top
            )
        })
        .collect();
    let pick = Select::new()
        .with_prompt("  target monitor")
        .items(&choices)
        .default(0)
        .interact()?;

    Ok(monitors[pick].index)
}

async fn cached_installed_apps() -> Result<Vec<crate::integrations::start_apps::StartApp>> {
    use std::sync::OnceLock;
    use tokio::sync::Mutex;

    static CACHE: OnceLock<Mutex<Option<Vec<crate::integrations::start_apps::StartApp>>>> =
        OnceLock::new();
    let cell = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().await;
    if guard.is_none() {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::with_template("  {spinner:.yellow} {msg}")?);
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        spinner.set_message("listing installed apps");
        let result = crate::integrations::start_apps::list_installed().await;
        spinner.finish_and_clear();
        *guard = Some(result?);
    }
    Ok(guard.as_ref().unwrap().clone())
}

fn prompt_browser_urls() -> Result<Vec<BrowserEntry>> {
    println!(
        "  {}",
        "send ravens to these URLs on activation — blank URL to finish".dimmed()
    );
    let mut urls = Vec::new();
    while let Some(entry) = prompt_one_browser_url()? {
        urls.push(entry);
    }
    Ok(urls)
}

fn prompt_one_browser_url() -> Result<Option<BrowserEntry>> {
    let url: String = Input::new()
        .with_prompt("  url")
        .allow_empty(true)
        .interact_text()?;
    if url.trim().is_empty() {
        return Ok(None);
    }
    let suggested = guess_url_label(&url);
    let name: String = Input::new()
        .with_prompt("  label")
        .default(suggested)
        .interact_text()?;
    Ok(Some(BrowserEntry::new(name, url)))
}

fn guess_url_label(url: &str) -> String {
    let s = url.trim();
    let no_proto = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let host = no_proto.split('/').next().unwrap_or(no_proto);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        "url".into()
    } else {
        host.to_string()
    }
}

fn prompt_optional_path(prompt: &str) -> Result<Option<String>> {
    let raw: String = Input::new()
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()?;
    Ok(if raw.trim().is_empty() {
        None
    } else {
        Some(raw)
    })
}

fn guess_label(cmd: &str) -> String {
    if launcher::looks_like_url(cmd) {
        return "url".into();
    }
    let path = PathBuf::from(cmd);
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| cmd.to_string())
}

/// Tiny space-splitter that respects double quotes. Good enough for wizard input.
fn shell_split(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in input.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            c if c.is_whitespace() && !in_quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn confirm(prompt: &str, default: bool) -> Result<bool> {
    Ok(Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}

/// Interactive menu-driven editor for an existing profile. Lets the user add
/// or remove startup apps, URLs, env vars, and edit metadata without leaving
/// the terminal. Saves on exit if the profile changed.
pub async fn edit_interactive(odin_dir: &Path, name: &str) -> Result<()> {
    let store = AsgardStore::new(odin_dir);
    let mut profile = store.load(name).await?;
    let original = profile.clone();

    loop {
        print_profile_overview(&profile);
        let choices = [
            "Add startup app",
            "Add browser URL",
            "Add environment variable",
            "Edit description",
            "Set VS Code workspace",
            "Edit a startup app",
            "Remove a startup app",
            "Remove a browser URL",
            "Remove an environment variable",
            "Save & exit",
            "Cancel (discard changes)",
        ];
        let pick = Select::new()
            .with_prompt("What next?")
            .items(&choices)
            .default(0)
            .interact()?;
        match pick {
            0 => {
                if let Some(app) = prompt_one_startup_app().await? {
                    profile.startup_apps.push(app);
                }
            }
            1 => {
                if let Some(entry) = prompt_one_browser_url()? {
                    profile.browser_urls.push(entry);
                }
            }
            2 => {
                let raw: String = Input::new()
                    .with_prompt("KEY=VALUE")
                    .allow_empty(true)
                    .interact_text()?;
                if let Some((k, v)) = raw.split_once('=') {
                    if !k.trim().is_empty() {
                        profile.env.insert(k.trim().to_string(), v.to_string());
                    } else {
                        println!("{} expected `KEY=VALUE`", "!".yellow());
                    }
                } else if !raw.trim().is_empty() {
                    println!("{} expected `KEY=VALUE`", "!".yellow());
                }
            }
            3 => {
                let desc: String = Input::new()
                    .with_prompt("Description")
                    .default(profile.description.clone())
                    .allow_empty(true)
                    .interact_text()?;
                profile.description = desc;
            }
            4 => {
                profile.vscode_workspace =
                    prompt_optional_path("VS Code workspace path (blank to clear)")?;
            }
            5 => {
                if profile.startup_apps.is_empty() {
                    println!("{} no startup apps to edit", "·".dimmed());
                } else {
                    let mut display: Vec<String> = profile
                        .startup_apps
                        .iter()
                        .map(|a| format!("{} ({})", a.name, a.command))
                        .collect();
                    display.push("(cancel)".to_string());
                    let pick_edit = Select::new()
                        .with_prompt("Which startup app to edit?")
                        .items(&display)
                        .default(display.len() - 1)
                        .interact()?;
                    if pick_edit < profile.startup_apps.len() {
                        let app = &mut profile.startup_apps[pick_edit];
                        println!("  {} {}", "Editing".cyan(), display[pick_edit]);
                        app.window = prompt_window_state()?;
                        app.layout = prompt_window_layout()?;
                    }
                }
            }
            6 => remove_indexed(&mut profile.startup_apps, "startup app", |a| {
                format!("{} ({})", a.name, a.command)
            })?,
            7 => remove_indexed(&mut profile.browser_urls, "URL", |u| {
                format!("{} ({})", u.name, u.url)
            })?,
            8 => remove_env_var(&mut profile.env)?,
            9 => {
                if profile == original {
                    println!("  {}  no changes", "·".dimmed());
                } else {
                    store.save(&profile).await?;
                    println!(
                        "  {}  realm {} saved",
                        "✓".green().bold(),
                        profile.name.bright_yellow().bold()
                    );
                }
                return Ok(());
            }
            _ => {
                println!("{} discarded changes", "·".dimmed());
                return Ok(());
            }
        }
    }
}

fn print_profile_overview(p: &Profile) {
    use std::io::Write;
    print!("\x1B[2J\x1B[H");
    let _ = std::io::stdout().flush();

    let rule: String = "─".repeat(58);
    println!();
    println!(
        "  {}  {}",
        " ᚨ ASGARD ".black().on_bright_yellow().bold(),
        p.name.bright_yellow().bold()
    );
    if !p.description.is_empty() {
        println!("  {}", p.description.italic().dimmed());
    }
    println!("  {}", rule.dimmed());

    if p.env.is_empty() {
        println!(
            "  {}  {}",
            "ᚱ runes  ".bright_yellow().bold(),
            "(none)".dimmed()
        );
    } else {
        println!(
            "  {}  {}",
            "ᚱ runes  ".bright_yellow().bold(),
            format!("{} variable(s)", p.env.len()).dimmed()
        );
        for (k, v) in &p.env {
            println!("          {} {} {}", k.bright_blue(), "=".dimmed(), v);
        }
    }

    println!();
    if p.startup_apps.is_empty() {
        println!(
            "  {}  {}",
            "⚒ warriors".bright_yellow().bold(),
            "(none)".dimmed()
        );
    } else {
        println!(
            "  {}  {}",
            "⚒ warriors".bright_yellow().bold(),
            format!("{} item(s)", p.startup_apps.len()).dimmed()
        );
        for (i, a) in p.startup_apps.iter().enumerate() {
            let extras = if a.args.is_empty() {
                String::new()
            } else {
                format!(" {}", a.args.join(" "))
            };
            let win = match a.window {
                WindowState::Normal => "".to_string(),
                WindowState::Minimized => "  [min]".to_string(),
                WindowState::Maximized => "  [max]".to_string(),
            };
            let lay = match &a.layout {
                Some(WindowLayout::Preset(p)) => format!("  [{:?}]", p),
                Some(WindowLayout::TargetedPreset { preset, monitor }) => {
                    format!("  [{preset:?} monitor {monitor}]")
                }
                Some(WindowLayout::Bounds { .. }) => "  [bounds]".to_string(),
                None => "".to_string(),
            };
            println!(
                "          {}  {}  {}{}{}{}",
                format!("{:>2}.", i + 1).dimmed(),
                a.name.bright_blue(),
                a.command,
                extras.dimmed(),
                win.dimmed(),
                lay.dimmed()
            );
        }
    }

    println!();
    if p.browser_urls.is_empty() {
        println!(
            "  {}  {}",
            "⌒ ravens ".bright_yellow().bold(),
            "(none)".dimmed()
        );
    } else {
        println!(
            "  {}  {}",
            "⌒ ravens ".bright_yellow().bold(),
            format!("{} item(s)", p.browser_urls.len()).dimmed()
        );
        for (i, u) in p.browser_urls.iter().enumerate() {
            println!(
                "          {}  {}  {}",
                format!("{:>2}.", i + 1).dimmed(),
                u.name.bright_magenta(),
                u.url.dimmed()
            );
        }
    }

    println!();
    match &p.vscode_workspace {
        Some(ws) => println!("  {}  {}", "◇ vscode ".bright_yellow().bold(), ws),
        None => println!(
            "  {}  {}",
            "◇ vscode ".bright_yellow().bold(),
            "(none)".dimmed()
        ),
    }

    println!("  {}", rule.dimmed());
    println!();
}

fn remove_indexed<T, F>(items: &mut Vec<T>, label: &str, render: F) -> Result<()>
where
    F: Fn(&T) -> String,
{
    if items.is_empty() {
        println!("{} no {label}s to remove", "·".dimmed());
        return Ok(());
    }
    let mut display: Vec<String> = items.iter().map(&render).collect();
    display.push("(cancel)".to_string());
    let pick = Select::new()
        .with_prompt(format!("Which {label} to remove?"))
        .items(&display)
        .default(display.len() - 1)
        .interact()?;
    if pick < items.len() {
        let removed = items.remove(pick);
        println!(
            "  {}  removed {}",
            "✓".green().bold(),
            render(&removed).cyan()
        );
    }
    Ok(())
}

fn remove_env_var(env: &mut BTreeMap<String, String>) -> Result<()> {
    if env.is_empty() {
        println!("{} no env vars to remove", "·".dimmed());
        return Ok(());
    }
    let keys: Vec<String> = env.keys().cloned().collect();
    let mut display = keys.clone();
    display.push("(cancel)".to_string());
    let pick = Select::new()
        .with_prompt("Which env var to remove?")
        .items(&display)
        .default(display.len() - 1)
        .interact()?;
    if pick < keys.len() {
        env.remove(&keys[pick]);
        println!(
            "  {}  removed {}",
            "✓".green().bold(),
            keys[pick].bright_blue().bold()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_split_basic() {
        assert_eq!(shell_split(""), Vec::<String>::new());
        assert_eq!(shell_split("a b c"), vec!["a", "b", "c"]);
        assert_eq!(
            shell_split(r#"new-tab -p "PowerShell""#),
            vec!["new-tab", "-p", "PowerShell"]
        );
        assert_eq!(
            shell_split(r#""C:\Program Files\app.exe" --foo"#),
            vec!["C:\\Program Files\\app.exe", "--foo"]
        );
    }

    #[test]
    fn first_line_trims_to_first() {
        assert_eq!(first_line("foo\nbar"), "foo");
        assert_eq!(first_line(""), "");
    }

    #[test]
    fn guess_label_url() {
        assert_eq!(guess_label("https://example.com"), "url");
    }

    #[test]
    fn guess_label_exe_path() {
        assert_eq!(guess_label("C:\\Windows\\notepad.exe"), "notepad");
        assert_eq!(guess_label("code"), "code");
    }
}
