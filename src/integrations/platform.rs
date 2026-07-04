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
        } else if resp.status().as_u16() == 429 {
            bail!(rate_limited_message(&resp));
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
        } else if resp.status().as_u16() == 429 {
            bail!(rate_limited_message(&resp));
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("snapshot upload failed with {status}: {body}");
        }
    }
}

/// Friendly message for a 429, including the server's `Retry-After` hint if present.
fn rate_limited_message(resp: &reqwest::Response) -> String {
    let retry = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(|s| format!(" — retry in {s}s"))
        .unwrap_or_default();
    format!("rate limited by the platform{retry}; try again shortly")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn request_device_code_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc_123",
                "user_code": "WXYZ-2345",
                "verification_uri": "https://x/activate",
                "verification_uri_complete": "https://x/activate?code=WXYZ-2345",
                "expires_in": 600,
                "interval": 5
            })))
            .mount(&server)
            .await;

        let client = PlatformClient::new(&server.uri()).unwrap();
        let resp = client.request_device_code(Some("laptop")).await.unwrap();
        assert_eq!(resp.device_code, "dc_123");
        assert_eq!(resp.user_code, "WXYZ-2345");
        assert_eq!(resp.interval, 5);
    }

    #[tokio::test]
    async fn poll_token_maps_pending_then_granted() {
        let server = MockServer::start().await;
        // Pending.
        Mock::given(method("POST"))
            .and(path("/api/device/token"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({ "error": "authorization_pending" })),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        let client = PlatformClient::new(&server.uri()).unwrap();
        assert!(matches!(
            client.poll_token("dc_123").await.unwrap(),
            PollOutcome::Pending
        ));

        // Granted (fresh server to swap the response).
        let server2 = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/device/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "odin_0123456789abcdef_secret",
                "token_type": "Bearer",
                "account": { "email": "ada@example.com" }
            })))
            .mount(&server2)
            .await;
        let client2 = PlatformClient::new(&server2.uri()).unwrap();
        match client2.poll_token("dc_123").await.unwrap() {
            PollOutcome::Granted {
                access_token,
                email,
            } => {
                assert_eq!(access_token, "odin_0123456789abcdef_secret");
                assert_eq!(email.as_deref(), Some("ada@example.com"));
            }
            other => panic!("expected Granted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn me_sends_bearer_and_parses_identity() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/cli/me"))
            .and(header("authorization", "Bearer odin_tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "userId": "u1",
                "email": "ada@example.com"
            })))
            .mount(&server)
            .await;
        let client = PlatformClient::new(&server.uri()).unwrap();
        let id = client.me("odin_tok").await.unwrap();
        assert_eq!(id.email.as_deref(), Some("ada@example.com"));
    }

    #[tokio::test]
    async fn me_401_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/cli/me"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Invalid token"
            })))
            .mount(&server)
            .await;
        let client = PlatformClient::new(&server.uri()).unwrap();
        assert!(client.me("bad").await.is_err());
    }

    #[tokio::test]
    async fn upload_snapshot_success_and_429() {
        // Success.
        let ok_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/ingest"))
            .and(header("authorization", "Bearer odin_tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "snapshotId": "snap-1"
            })))
            .mount(&ok_server)
            .await;
        let client = PlatformClient::new(&ok_server.uri()).unwrap();
        let payload = serde_json::json!({ "lock": { "snapshot_id": "snap-1" } });
        let resp = client.upload_snapshot("odin_tok", &payload).await.unwrap();
        assert_eq!(resp.snapshot_id.as_deref(), Some("snap-1"));

        // Rate limited.
        let limited = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/ingest"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "42")
                    .set_body_json(serde_json::json!({ "error": "rate_limited" })),
            )
            .mount(&limited)
            .await;
        let client2 = PlatformClient::new(&limited.uri()).unwrap();
        let err = client2
            .upload_snapshot("odin_tok", &payload)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("rate limited"), "got: {err}");
        assert!(err.contains("42"), "should surface retry-after: {err}");
    }

    #[tokio::test]
    async fn token_is_sent_verbatim_regardless_of_shape() {
        // Locks in the "no lockstep needed" property: whatever opaque string we
        // store, the client replays it unchanged in the Authorization header.
        for token in [
            "odin_legacyhex",
            "odin_0123456789abcdef_secret",
            "weird.token",
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/cli/me"))
                .and(header("authorization", format!("Bearer {token}").as_str()))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
                .mount(&server)
                .await;
            let client = PlatformClient::new(&server.uri()).unwrap();
            assert!(
                client.me(token).await.is_ok(),
                "token {token} was not sent verbatim"
            );
        }
    }
}
