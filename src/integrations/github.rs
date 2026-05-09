use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct GitHubClient {
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct CreateRepoRequest<'a> {
    name: &'a str,
    private: bool,
    description: &'a str,
    auto_init: bool,
}

#[derive(Debug, Deserialize)]
pub struct GitHubRepository {
    pub clone_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub login: String,
}

impl GitHubClient {
    pub fn new(token: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("odin-cli"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .context("invalid GitHub token header")?,
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self { client })
    }

    pub async fn create_user_repo(
        &self,
        repo: &str,
        private: bool,
        description: &str,
    ) -> Result<GitHubRepository> {
        let response = self
            .client
            .post("https://api.github.com/user/repos")
            .json(&CreateRepoRequest {
                name: repo,
                private,
                description,
                auto_init: false,
            })
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json::<GitHubRepository>().await?)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GitHub repository creation failed with {status}: {body}");
        }
    }

    pub async fn current_user(&self) -> Result<GitHubUser> {
        let response = self
            .client
            .get("https://api.github.com/user")
            .send()
            .await?;
        if response.status().is_success() {
            Ok(response.json::<GitHubUser>().await?)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GitHub authentication failed with {status}: {body}");
        }
    }
}
