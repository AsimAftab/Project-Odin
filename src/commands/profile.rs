use std::path::Path;

use anyhow::{anyhow, bail, Result};
use colored::Colorize;
use dialoguer::Confirm;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Archive, Builder};

use crate::asgard::profile::{validate_name, Profile};
use crate::asgard::store::{AsgardStore, PROFILE_FILE};
use crate::cli::{
    ProfileArgs, ProfileCommands, ProfileCreateArgs, ProfileDeleteArgs, ProfileEditArgs,
    ProfileExportArgs, ProfileImportArgs, ProfileListArgs,
};
use crate::core::context::AppContext;
use crate::services::asgard_service;
use crate::utils::terminal;

pub async fn run(ctx: AppContext, args: ProfileArgs) -> Result<()> {
    match args.command {
        ProfileCommands::List(a) => list(ctx, a).await,
        ProfileCommands::Create(a) => create(ctx, a).await,
        ProfileCommands::Delete(a) => delete(ctx, a).await,
        ProfileCommands::Edit(a) => edit(ctx, a).await,
        ProfileCommands::Export(a) => export(ctx, a).await,
        ProfileCommands::Import(a) => import(ctx, a).await,
    }
}

async fn list(ctx: AppContext, args: ProfileListArgs) -> Result<()> {
    let store = AsgardStore::new(ctx.odin_dir());
    let summaries = store.list_summaries().await?;
    let active = store.load_state().await?.active_profile;

    if args.json {
        let payload = serde_json::json!({
            "active": active,
            "profiles": summaries,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if summaries.is_empty() {
        println!(
            "{} no profiles yet — run `odin activate asgard` to create one",
            "·".dimmed()
        );
        return Ok(());
    }

    println!("{}", "Asgard Profiles".bold());
    for s in summaries {
        let marker = if active.as_deref() == Some(s.name.as_str()) {
            "● ".green()
        } else {
            "  ".normal()
        };
        let extras = format!(
            "apps:{} urls:{}{}",
            s.startup_app_count,
            s.browser_url_count,
            if s.has_vscode { " vscode" } else { "" }
        );
        if s.description.is_empty() {
            println!("{}{}  {}", marker, s.name.cyan(), extras.dimmed());
        } else {
            println!(
                "{}{}  {} — {}",
                marker,
                s.name.cyan(),
                extras.dimmed(),
                s.description
            );
        }
    }
    Ok(())
}

async fn create(ctx: AppContext, args: ProfileCreateArgs) -> Result<()> {
    if !terminal::is_interactive() {
        bail!(
            "`odin profile create` requires a TTY; pre-author a YAML and use `odin profile import`"
        );
    }
    let profile = asgard_service::wizard(ctx.odin_dir(), args.name).await?;
    if asgard_service::confirm("Activate it now?", true)? {
        let report = asgard_service::activate(ctx.odin_dir(), &profile.name).await?;
        asgard_service::print_activation_report(&report);
    }
    Ok(())
}

async fn delete(ctx: AppContext, args: ProfileDeleteArgs) -> Result<()> {
    if !args.force && terminal::is_interactive() {
        let ok = Confirm::new()
            .with_prompt(format!("delete profile `{}`?", args.name))
            .default(false)
            .interact()?;
        if !ok {
            println!("{} cancelled", "·".dimmed());
            return Ok(());
        }
    }
    asgard_service::delete(ctx.odin_dir(), &args.name).await?;
    println!("{} deleted {}", "ok".green(), args.name.cyan());
    Ok(())
}

async fn edit(ctx: AppContext, args: ProfileEditArgs) -> Result<()> {
    let store = AsgardStore::new(ctx.odin_dir());
    if !store.exists(&args.name) {
        bail!("profile `{}` not found", args.name);
    }
    if !terminal::is_interactive() {
        bail!("`odin profile edit` requires a TTY");
    }
    asgard_service::edit_interactive(ctx.odin_dir(), &args.name).await
}

async fn export(ctx: AppContext, args: ProfileExportArgs) -> Result<()> {
    let store = AsgardStore::new(ctx.odin_dir());
    if !store.exists(&args.name) {
        bail!("profile `{}` not found", args.name);
    }
    let dir = store.profile_dir(&args.name);
    let out = args
        .out
        .unwrap_or_else(|| std::path::PathBuf::from(format!("{}.tar.gz", args.name)));

    let name = args.name.clone();
    let dir_clone = dir.clone();
    let out_clone = out.clone();
    tokio::task::spawn_blocking(move || tar_gz_dir(&dir_clone, &name, &out_clone)).await??;

    println!(
        "{} exported {} to {}",
        "ok".green(),
        args.name.cyan(),
        out.display()
    );
    Ok(())
}

async fn import(ctx: AppContext, args: ProfileImportArgs) -> Result<()> {
    let store = AsgardStore::new(ctx.odin_dir());
    store.ensure().await?;
    let archive_path = args.path.clone();
    let root = store.root().to_path_buf();

    let extracted_name =
        tokio::task::spawn_blocking(move || untar_gz(&archive_path, &root)).await??;
    validate_name(&extracted_name).map_err(|e| anyhow!(e))?;

    let profile: Profile = store.load(&extracted_name).await?;
    if profile.name != extracted_name {
        bail!(
            "archive directory `{extracted_name}` doesn't match profile name `{}` in YAML",
            profile.name
        );
    }
    if !args.force
        && store
            .list()
            .await?
            .iter()
            .filter(|n| *n == &extracted_name)
            .count()
            > 1
    {
        bail!(
            "profile `{extracted_name}` already exists; pass --force to overwrite (note: existing files were preserved)"
        );
    }

    println!("{} imported {}", "ok".green(), profile.name.cyan());
    Ok(())
}

fn tar_gz_dir(dir: &Path, name: &str, out: &Path) -> Result<()> {
    let file = std::fs::File::create(out)?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);
    tar.append_dir_all(name, dir)?;
    tar.finish()?;
    Ok(())
}

fn untar_gz(archive: &Path, root: &Path) -> Result<String> {
    let file = std::fs::File::open(archive)?;
    let dec = GzDecoder::new(file);
    let mut tar = Archive::new(dec);
    tar.unpack(root)?;

    // Find the freshly unpacked directory containing PROFILE_FILE.
    let mut found = None;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if dir_name.starts_with('.') {
            continue;
        }
        if entry.path().join(PROFILE_FILE).exists() {
            // Pick the most recently modified — the just-extracted one.
            let mtime = entry.metadata()?.modified().ok();
            match &found {
                None => found = Some((dir_name, mtime)),
                Some((_, prev_mtime)) if mtime > *prev_mtime => {
                    found = Some((dir_name, mtime));
                }
                _ => {}
            }
        }
    }
    found
        .map(|(n, _)| n)
        .ok_or_else(|| anyhow!("archive did not contain a profile.yaml at the top level"))
}
