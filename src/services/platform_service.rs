//! Orchestrates the Odin Platform connection: device-flow login, snapshot
//! payload assembly (with secret redaction), and uploads. Uploads are always
//! non-fatal to local state — this service never modifies snapshot files.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::asgard::store::AsgardStore;
use crate::integrations::platform::{Identity, PlatformClient, PollOutcome};
use crate::integrations::process;
use crate::models::config::OdinConfig;
use crate::services::config_service::ConfigService;
use crate::services::redact;
use crate::services::secret_service::SecretService;
use crate::services::storage::SnapshotStore;

pub struct PlatformService {
    odin_dir: PathBuf,
}

pub struct LoginResult {
    pub url: String,
    pub email: Option<String>,
}

pub struct BackfillSummary {
    pub uploaded: usize,
    pub failed: usize,
    pub total: usize,
}

impl PlatformService {
    pub fn new(odin_dir: PathBuf) -> Self {
        Self { odin_dir }
    }

    /// Runs the OAuth 2.0 Device Authorization flow end to end: request a code,
    /// show/open the verification URL, poll until approved, then persist the
    /// token (keyring) and platform URL (config.yaml).
    pub async fn login(
        &self,
        url: &str,
        label: Option<&str>,
        open_browser: bool,
    ) -> Result<LoginResult> {
        let url = url.trim().trim_end_matches('/').to_string();
        if url.is_empty() {
            bail!("a platform URL is required");
        }
        let client = PlatformClient::new(&url)?;
        let device = client.request_device_code(label).await?;

        println!();
        println!(
            "  {}  Open this URL to approve the device:",
            "→".bright_blue()
        );
        println!("      {}", device.verification_uri_complete.cyan().bold());
        println!(
            "  {}  Verification code: {}",
            "·".dimmed(),
            device.user_code.bright_yellow().bold()
        );
        println!();

        if open_browser {
            // Best-effort — if it fails the user still has the URL above.
            let _ = process::open_url(&device.verification_uri_complete).await;
        }

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template("  {spinner:.yellow} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        spinner.enable_steady_tick(Duration::from_millis(90));
        spinner.set_message("Waiting for approval in the browser…");

        let start = Instant::now();
        let expires = Duration::from_secs(device.expires_in.max(1));
        let mut interval = device.interval.max(1);

        let result = loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if start.elapsed() >= expires {
                break Err(anyhow::anyhow!(
                    "login timed out — the code expired before approval. Run `odin login` again."
                ));
            }
            match client.poll_token(&device.device_code).await {
                Ok(PollOutcome::Pending) => continue,
                Ok(PollOutcome::SlowDown) => {
                    interval += 5;
                    continue;
                }
                Ok(PollOutcome::Denied) => {
                    break Err(anyhow::anyhow!("login was denied in the browser."));
                }
                Ok(PollOutcome::Expired) => {
                    break Err(anyhow::anyhow!(
                        "login timed out — the code expired. Run `odin login` again."
                    ));
                }
                Ok(PollOutcome::Granted {
                    access_token,
                    email,
                }) => break Ok((access_token, email)),
                Err(e) => break Err(e),
            }
        };
        spinner.finish_and_clear();

        let (access_token, email) = result?;
        self.store_credentials(&url, &access_token).await?;
        Ok(LoginResult { url, email })
    }

    /// Stores the token in the OS keyring and records the platform URL + token
    /// key in config.yaml (the raw token never touches config.yaml).
    pub async fn store_credentials(&self, url: &str, token: &str) -> Result<()> {
        let key = SecretService::platform_token_key(url);
        SecretService::set_token(&key, token)?;

        let service = ConfigService::new(self.odin_dir.clone());
        let mut config = service.load().await?;
        config.platform.url = Some(url.to_string());
        config.platform.token_key = Some(key);
        service.save(&config).await?;
        Ok(())
    }

    /// Enables/disables automatic upload after each snapshot.
    pub async fn set_upload_on_snapshot(&self, enabled: bool) -> Result<()> {
        let service = ConfigService::new(self.odin_dir.clone());
        let mut config = service.load().await?;
        config.platform.upload_on_snapshot = enabled;
        service.save(&config).await?;
        Ok(())
    }

    /// Clears the stored token and platform config (idempotent).
    pub async fn logout(&self) -> Result<()> {
        let service = ConfigService::new(self.odin_dir.clone());
        let mut config = service.load().await?;
        if let Some(key) = config.platform.token_key.take() {
            let _ = SecretService::delete_token(&key);
        }
        config.platform.url = None;
        config.platform.upload_on_snapshot = false;
        service.save(&config).await?;
        Ok(())
    }

    /// Verifies the stored token against the platform and returns identity.
    pub async fn verify(&self, config: &OdinConfig) -> Result<Identity> {
        let (url, token) = require_config(config)?;
        PlatformClient::new(&url)?.me(&token).await
    }

    /// Uploads the current (latest) snapshot in the vault root.
    pub async fn upload_latest(&self, config: &OdinConfig) -> Result<String> {
        let (url, token) = require_config(config)?;
        let store = SnapshotStore::new(self.odin_dir.clone());
        let profiles = self.profiles_section().await;
        let payload = build_payload(&store, profiles.as_ref())
            .await
            .context("no local snapshot to upload — run `odin snapshot` first")?;
        let resp = PlatformClient::new(&url)?
            .upload_snapshot(&token, &payload)
            .await?;
        Ok(resp.snapshot_id.unwrap_or_default())
    }

    /// Builds the optional Asgard profiles summary for the ingest payload:
    /// profile names/descriptions/app names plus the active profile. No
    /// commands, args, or env values leave the machine (privacy). Returns None
    /// when there are no profiles or the store can't be read — the upload
    /// simply omits the key.
    async fn profiles_section(&self) -> Option<serde_json::Value> {
        let store = AsgardStore::new(&self.odin_dir);
        let names = store.list().await.ok()?;
        if names.is_empty() {
            return None;
        }

        let mut profiles = Vec::new();
        for name in &names {
            let Ok(p) = store.load(name).await else {
                continue;
            };
            profiles.push(serde_json::json!({
                "name": p.name,
                "description": p.description,
                "startup_app_count": p.startup_apps.len(),
                "browser_url_count": p.browser_urls.len(),
                "has_vscode": p.vscode_workspace.is_some(),
                "app_names": p.startup_apps.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
            }));
        }
        if profiles.is_empty() {
            return None;
        }

        let state = store.load_state().await.unwrap_or_default();
        Some(serde_json::json!({
            "profiles": profiles,
            "active_profile": state.active_profile,
            "activated_at": state.activated_at,
        }))
    }

    /// Uploads every snapshot found under `~/.odin/history`. Idempotent on the
    /// platform (keyed by snapshot_id), and resilient: a snapshot that fails to
    /// read or upload is counted and skipped, never aborting the batch.
    pub async fn upload_all_history(&self, config: &OdinConfig) -> Result<BackfillSummary> {
        let (url, token) = require_config(config)?;
        let client = PlatformClient::new(&url)?;

        let history_root = self.odin_dir.join("history");
        let mut dirs: Vec<PathBuf> = Vec::new();
        if history_root.exists() {
            let mut rd = tokio::fs::read_dir(&history_root).await?;
            while let Some(entry) = rd.next_entry().await? {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    dirs.push(entry.path());
                }
            }
        }

        let total = dirs.len();
        let mut uploaded = 0;
        let mut failed = 0;

        let bar = ProgressBar::new(total as u64);
        bar.set_style(
            ProgressStyle::with_template("  {bar:30.yellow/dim} {pos}/{len} snapshots")
                .unwrap_or_else(|_| ProgressStyle::default_bar()),
        );

        let profiles = self.profiles_section().await;
        for dir in dirs {
            let store = SnapshotStore::new(dir.clone());
            match build_payload(&store, profiles.as_ref()).await {
                Ok(payload) => match client.upload_snapshot(&token, &payload).await {
                    Ok(_) => uploaded += 1,
                    Err(e) => {
                        failed += 1;
                        tracing::warn!("failed to upload {}: {e}", dir.display());
                    }
                },
                Err(e) => {
                    failed += 1;
                    tracing::warn!("skipping unreadable snapshot {}: {e}", dir.display());
                }
            }
            bar.inc(1);
        }
        bar.finish_and_clear();

        Ok(BackfillSummary {
            uploaded,
            failed,
            total,
        })
    }
}

/// True when the platform is configured (URL + token key present).
pub fn is_configured(config: &OdinConfig) -> bool {
    config.platform.url.is_some() && config.platform.token_key.is_some()
}

/// Resolves the platform URL and API token, or a clear "run `odin login`" error.
fn require_config(config: &OdinConfig) -> Result<(String, String)> {
    let url = config
        .platform
        .url
        .clone()
        .context("not connected to a platform — run `odin login` first")?;
    let key = config
        .platform
        .token_key
        .as_deref()
        .context("no platform token configured — run `odin login` first")?;
    let token = SecretService::get_token(key)?;
    Ok((url, token))
}

/// Builds the `/api/ingest` payload from a snapshot store, redacting secrets in
/// the environment section. Requires all six snapshot files to be present.
/// `profiles` is the optional Asgard summary attached to every upload.
async fn build_payload(
    store: &SnapshotStore,
    profiles: Option<&serde_json::Value>,
) -> Result<serde_json::Value> {
    let machine = store.read_machine().await?;
    let environment = redact::redact_environment(store.read_environment().await?);
    let packages = store.read_packages().await?;
    let vscode = store.read_vscode().await?;
    let git = store.read_git().await?;
    let lock = store.read_lock().await?;

    let mut map = serde_json::Map::new();
    map.insert("machine".into(), serde_json::to_value(&machine)?);
    map.insert("environment".into(), serde_json::to_value(&environment)?);
    map.insert("packages".into(), serde_json::to_value(&packages)?);
    map.insert("vscode".into(), serde_json::to_value(&vscode)?);
    map.insert("git".into(), serde_json::to_value(&git)?);
    map.insert("lock".into(), serde_json::to_value(&lock)?);
    if let Some(profiles) = profiles {
        map.insert("profiles".into(), profiles.clone());
    }
    Ok(serde_json::Value::Object(map))
}
