use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::models::plugin::{InstalledPlugin, PluginManifest, PluginResult};

const MANIFEST_FILE: &str = "plugin.toml";
const INSTALLED_META: &str = ".odin-installed.json";

pub fn manifest_path(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(MANIFEST_FILE)
}

pub fn installed_meta_path(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(INSTALLED_META)
}

pub fn load_manifest(plugin_dir: &Path) -> Result<PluginManifest> {
    let path = manifest_path(plugin_dir);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("reading manifest at {}", path.display()))?;
    let manifest: PluginManifest = toml::from_str(&content)
        .with_context(|| format!("parsing manifest at {}", path.display()))?;
    Ok(manifest)
}

pub fn read_installed(plugin_dir: &Path) -> Result<Option<InstalledPlugin>> {
    let path = installed_meta_path(plugin_dir);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let installed: InstalledPlugin =
        serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(installed))
}

pub fn write_installed(plugin_dir: &Path, installed: &InstalledPlugin) -> Result<()> {
    let path = installed_meta_path(plugin_dir);
    let content = serde_json::to_string_pretty(installed)?;
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn execute(installed: &InstalledPlugin, args: &[String]) -> Result<PluginResult> {
    let executable = installed.install_path.join(&installed.manifest.executable);
    if !executable.exists() {
        anyhow::bail!("plugin executable not found: {}", executable.display());
    }
    let output = std::process::Command::new(&executable)
        .args(args)
        .output()
        .with_context(|| format!("running {}", executable.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code().unwrap_or(-1);
    Ok(PluginResult {
        success: output.status.success(),
        stdout,
        stderr,
        exit_code,
    })
}
