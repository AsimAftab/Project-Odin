use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::integrations::github;

pub enum UpdateOutcome {
    UpToDate { current: String, latest: String },
    UpdateAvailable { current: String, latest: String },
    UpdateStaged { current: String, latest: String },
}

pub struct UpdateService;

impl UpdateService {
    pub async fn run(check_only: bool) -> Result<UpdateOutcome> {
        let current = env!("CARGO_PKG_VERSION").to_string();
        let (owner, repo) = resolve_release_repo()?;
        let release = github::latest_release(&owner, &repo).await?;
        let latest = trim_version_prefix(&release.tag_name).to_string();

        if !is_newer_version(&latest, &current) {
            return Ok(UpdateOutcome::UpToDate {
                current,
                latest: release.tag_name,
            });
        }
        if check_only {
            return Ok(UpdateOutcome::UpdateAvailable { current, latest });
        }

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name.eq_ignore_ascii_case("odin.exe"))
            .context("latest release does not contain odin.exe asset")?;
        let bytes = reqwest::get(&asset.browser_download_url)
            .await?
            .bytes()
            .await?;

        let target =
            std::env::current_exe().context("failed to resolve current executable path")?;
        let source = download_to_temp(&bytes).await?;
        stage_replace_after_exit(&source, &target).await?;

        Ok(UpdateOutcome::UpdateStaged { current, latest })
    }
}

async fn download_to_temp(bytes: &[u8]) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("odin-update-{}.exe", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::File::create(&path)
        .await
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(bytes).await?;
    file.flush().await?;
    Ok(path)
}

async fn stage_replace_after_exit(source: &Path, target: &Path) -> Result<()> {
    let script_path =
        std::env::temp_dir().join(format!("odin-updater-{}.ps1", uuid::Uuid::new_v4()));
    let backup = target.with_extension("old.exe");
    let script = "$ErrorActionPreference = 'Stop'\n\
         param([int]$CurrentPid, [string]$SourcePath, [string]$TargetPath, [string]$BackupPath)\n\
         while (Get-Process -Id $CurrentPid -ErrorAction SilentlyContinue) { Start-Sleep -Milliseconds 250 }\n\
         if (Test-Path $BackupPath) { Remove-Item $BackupPath -Force }\n\
         if (Test-Path $TargetPath) { Move-Item $TargetPath $BackupPath -Force }\n\
         Move-Item $SourcePath $TargetPath -Force\n\
         if (Test-Path $BackupPath) { Remove-Item $BackupPath -Force }\n\
         Remove-Item $MyInvocation.MyCommand.Path -Force\n"
        .to_string();
    tokio::fs::write(&script_path, script).await?;

    let current_pid = std::process::id().to_string();
    let child = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(script_path.to_string_lossy().as_ref())
        .arg("-CurrentPid")
        .arg(current_pid)
        .arg("-SourcePath")
        .arg(source.to_string_lossy().as_ref())
        .arg("-TargetPath")
        .arg(target.to_string_lossy().as_ref())
        .arg("-BackupPath")
        .arg(backup.to_string_lossy().as_ref())
        .spawn()
        .context("failed to launch update worker process")?;
    let _ = child.id();
    Ok(())
}

fn resolve_release_repo() -> Result<(String, String)> {
    if let Ok(repository) = std::env::var("ODIN_RELEASE_REPO") {
        let trimmed = repository.trim().trim_matches('/');
        if let Some((owner, repo)) = trimmed.split_once('/') {
            return Ok((owner.to_string(), repo.to_string()));
        }
    }

    let repository_url = env!("CARGO_PKG_REPOSITORY");
    let path = repository_url
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")
        .or_else(|| {
            repository_url
                .trim_end_matches('/')
                .strip_prefix("http://github.com/")
        })
        .context("failed to parse GitHub repository URL for updater")?;
    let mut parts = path.split('/');
    let owner = parts
        .next()
        .context("missing repository owner in updater repository URL")?;
    let repo = parts
        .next()
        .context("missing repository name in updater repository URL")?;
    Ok((owner.to_string(), repo.to_string()))
}

fn trim_version_prefix(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

fn parse_version_numbers(input: &str) -> Vec<u64> {
    trim_version_prefix(input)
        .split(['.', '-'])
        .map(|segment| segment.parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_newer_version(candidate: &str, current: &str) -> bool {
    let mut a = parse_version_numbers(candidate);
    let mut b = parse_version_numbers(current);
    let max = a.len().max(b.len());
    a.resize(max, 0);
    b.resize(max, 0);
    a > b
}
