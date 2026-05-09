use std::path::Path;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

pub async fn ensure_dir(path: &Path) -> Result<()> {
    tokio::fs::create_dir_all(path).await?;
    Ok(())
}

pub async fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent).await?;
    }
    let data = serde_json::to_vec_pretty(value)?;
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(&data).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

pub async fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let data = tokio::fs::read(path).await?;
    Ok(serde_json::from_slice(&data)?)
}

pub async fn write_text(path: &Path, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent).await?;
    }
    tokio::fs::write(path, value).await?;
    Ok(())
}
