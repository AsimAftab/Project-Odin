use anyhow::Result;
use chrono::Utc;
use colored::Colorize;

use crate::core::errors::OdinError;
use crate::integrations::{git_cli, github::GitHubClient, process};
use crate::services::storage::SnapshotStore;

pub struct SyncService {
    store: SnapshotStore,
}

impl SyncService {
    pub fn new(store: SnapshotStore) -> Self {
        Self { store }
    }

    pub async fn sync(
        &self,
        mut remote: Option<String>,
        create_private_repo: bool,
        github_repo: Option<String>,
        github_token: Option<String>,
        branch: &str,
        message: Option<String>,
    ) -> Result<()> {
        let Some(git) = git_cli::executable() else {
            return Err(OdinError::MissingCommand("git".to_string()).into());
        };

        self.ensure_repo(remote.clone(), branch).await?;
        let root = self.store.root().to_string_lossy().to_string();

        if create_private_repo {
            let token = github_token
                .or_else(|| std::env::var("GITHUB_TOKEN").ok())
                .ok_or_else(|| {
                    anyhow::anyhow!("--create-private-repo requires --github-token or GITHUB_TOKEN")
                })?;
            let repo = github_repo
                .ok_or_else(|| anyhow::anyhow!("--create-private-repo requires --github-repo"))?;
            let created = GitHubClient::new(&token)?
                .create_user_repo(&repo, true, "Private Odin developer environment snapshots")
                .await?;
            remote = Some(created.clone_url);
        }

        if let Some(remote_url) = remote {
            let remotes = process::capture(&git, &["-C", &root, "remote"]).await?;
            if remotes.stdout.lines().any(|line| line == "origin") {
                process::checked(
                    &git,
                    &["-C", &root, "remote", "set-url", "origin", &remote_url],
                )
                .await?;
            } else {
                process::checked(&git, &["-C", &root, "remote", "add", "origin", &remote_url])
                    .await?;
            }
        }

        process::checked(&git, &["-C", &root, "add", "."]).await?;
        let status = process::capture(&git, &["-C", &root, "status", "--porcelain"]).await?;
        if status.stdout.trim().is_empty() {
            println!();
            println!(
                "  {}  realm matches the vault — nothing to send across the Bifrost",
                "·".dimmed()
            );
            println!();
            return Ok(());
        }

        let msg = message.unwrap_or_else(|| {
            format!(
                "Odin snapshot {}",
                Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
            )
        });
        process::checked(&git, &["-C", &root, "commit", "-m", &msg]).await?;
        process::checked(&git, &["-C", &root, "branch", "-M", branch]).await?;
        process::checked(&git, &["-C", &root, "push", "-u", "origin", branch]).await?;
        println!();
        println!(
            "  {}  Bifrost crossed — pushed to branch {}",
            "✓".green().bold(),
            branch.bright_yellow().bold()
        );
        println!();
        Ok(())
    }

    pub async fn ensure_repo(&self, remote: Option<String>, branch: &str) -> Result<()> {
        let Some(git) = git_cli::executable() else {
            return Err(OdinError::MissingCommand("git".to_string()).into());
        };
        git_cli::init_repo(self.store.root()).await?;
        let root = self.store.root().to_string_lossy().to_string();
        process::checked(&git, &["-C", &root, "branch", "-M", branch]).await?;

        if let Some(remote_url) = remote {
            let remotes = process::capture(&git, &["-C", &root, "remote"]).await?;
            if remotes.stdout.lines().any(|line| line == "origin") {
                process::checked(
                    &git,
                    &["-C", &root, "remote", "set-url", "origin", &remote_url],
                )
                .await?;
            } else {
                process::checked(&git, &["-C", &root, "remote", "add", "origin", &remote_url])
                    .await?;
            }
        }
        Ok(())
    }
}
