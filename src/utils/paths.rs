use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::UserDirs;

pub fn default_odin_dir() -> Result<PathBuf> {
    let dirs = UserDirs::new().context("could not resolve user profile directory")?;
    Ok(dirs.home_dir().join(".odin"))
}

pub fn user_profile() -> Result<PathBuf> {
    let dirs = UserDirs::new().context("could not resolve user profile directory")?;
    Ok(dirs.home_dir().to_path_buf())
}
