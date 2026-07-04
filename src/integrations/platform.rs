//! HTTP client for the Odin Platform.
//!
//! Mirrors `integrations::github` in shape. Covers the OAuth 2.0 Device
//! Authorization flow (RFC 8628) used by `odin login`, a token verification
//! probe, and snapshot upload to `POST /api/ingest`. All networking for the
//! platform lives here; orchestration is in `services::platform_service`.

use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PlatformClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    #[allow(dead_code)]
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Result of a single poll of the device-token endpoint.
#[derive(Debug)]
pub enum PollOutcome {
    Pending,
    SlowDown,
    Granted {
        access_token: String,
        email: Option<String>,
    },
    Denied,
    Expired,
}

#[derive(Debug, Deserialize)]
pub struct Identity {
    #[serde(default, rename = "userId")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IngestResponse {
    #[serde(default, rename = "snapshotId")]
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct DeviceCodeRequest<'a> {
    label: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct DeviceTokenRequest<'a> {
    device_code: &'a str,
}

#[derive(Debug, Deserialize)]
struct TokenSuccess {
    access_token: String,
    #[serde(default)]
    account: Option<Account>,
}

#[derive(Debug, Deserialize)]
struct Account {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: Option<String>,
}

impl PlatformClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("odin-cli"));
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// RFC 8628 step 1 — request a device_code + user_code pair.
    pub async fn request_device_code(&self, label: Option<&str>) -> Result<DeviceCodeResponse> {
        let resp = self
            .client
            .post(self.url("/api/device/code"))
            .json(&DeviceCodeRequest { label })
            .send()
            .await
            .context("failed to reach the platform — check the URL and your connection")?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("device authorization request failed with {status}: {body}");
        }
    }

    /// RFC 8628 step 3 — poll once for the access token.
    pub async fn poll_token(&self, device_code: &str) -> Result<PollOutcome> {
        let resp = self
            .client
            .post(self.url("/api/device/token"))
            .json(&DeviceTokenRequest { device_code })
            .send()
            .await
            .context("failed to reach the platform while polling for approval")?;

        if resp.status().is_success() {
            let ok: TokenSuccess = resp.json().await?;
            let email = ok.account.and_then(|a| a.email);
            return Ok(PollOutcome::Granted {
                access_token: ok.access_token,
                email,
            });
        }

        let err: ErrorBody = resp.json().await.unwrap_or(ErrorBody { error: None });
        match err.error.as_deref() {
            Some("authorization_pending") => Ok(PollOutcome::Pending),
            Some("slow_down") => Ok(PollOutcome::SlowDown),
            Some("access_denied") => Ok(PollOutcome::Denied),
            Some("expired_token") => Ok(PollOutcome::Expired),
            other => bail!(
                "device token poll failed: {}",
                other.unwrap_or("unknown error")
            ),
        }
    }

    /// Verifies a token and returns the account identity (`GET /api/cli/me`).
    pub async fn me(&self, token: &str) -> Result<Identity> {
        let resp = self
            .client
            .get(self.url("/api/cli/me"))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .context("failed to reach the platform")?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("token verification failed with {status}: {body}");
        }
    }

    /// Uploads one snapshot payload to `POST /api/ingest`.
    pub async fn upload_snapshot(
        &self,
        token: &str,
        payload: &serde_json::Value,
    ) -> Result<IngestResponse> {
        let resp = self
            .client
            .post(self.url("/api/ingest"))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json")
            .json(payload)
            .send()
            .await
            .context("failed to reach the platform ingest endpoint")?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("snapshot upload failed with {status}: {body}");
        }
    }
}
