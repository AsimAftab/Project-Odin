use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::integrations::plugin_loader;
use crate::models::plugin::{InstalledPlugin, PluginResult};

const PLUGINS_SUBDIR: &str = "plugins";

pub struct PluginService {
    plugins_root: PathBuf,
}

impl PluginService {
    pub fn new(odin_dir: &Path) -> Self {
        Self {
            plugins_root: odin_dir.join(PLUGINS_SUBDIR),
        }
    }

    pub fn install(&self, source: &Path) -> Result<InstalledPlugin> {
        if !source.is_dir() {
            anyhow::bail!("source must be a directory: {}", source.display());
        }
        let manifest = plugin_loader::load_manifest(source)
            .with_context(|| format!("loading manifest from {}", source.display()))?;
        let install_path = self.plugins_root.join(&manifest.name);
        if install_path.exists() {
            anyhow::bail!(
                "plugin '{}' is already installed at {}; remove it first or pick a different name",
                manifest.name,
                install_path.display()
            );
        }
        std::fs::create_dir_all(&install_path)?;
        copy_dir_contents(source, &install_path)?;
        let installed = InstalledPlugin::new(manifest, install_path.clone());
        plugin_loader::write_installed(&install_path, &installed)?;
        Ok(installed)
    }

    pub fn list(&self) -> Result<Vec<InstalledPlugin>> {
        if !self.plugins_root.exists() {
            return Ok(Vec::new());
        }
        let mut plugins = Vec::new();
        for entry in std::fs::read_dir(&self.plugins_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let dir = entry.path();
            if let Some(installed) = plugin_loader::read_installed(&dir)? {
                plugins.push(installed);
            } else if plugin_loader::manifest_path(&dir).exists() {
                let manifest = plugin_loader::load_manifest(&dir)?;
                plugins.push(InstalledPlugin::new(manifest, dir));
            }
        }
        plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        Ok(plugins)
    }

    pub fn find(&self, name: &str) -> Result<InstalledPlugin> {
        let dir = self.plugins_root.join(name);
        if !dir.exists() {
            anyhow::bail!("plugin '{}' not installed", name);
        }
        match plugin_loader::read_installed(&dir)? {
            Some(installed) => Ok(installed),
            None => {
                let manifest = plugin_loader::load_manifest(&dir)?;
                Ok(InstalledPlugin::new(manifest, dir))
            }
        }
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut installed = self.find(name)?;
        installed.enabled = enabled;
        plugin_loader::write_installed(&installed.install_path, &installed)?;
        Ok(())
    }

    pub fn run(&self, name: &str, args: &[String]) -> Result<PluginResult> {
        let installed = self.find(name)?;
        if !installed.enabled {
            anyhow::bail!(
                "plugin '{}' is disabled; run `odin plugin enable {}` to enable it",
                name,
                name
            );
        }
        plugin_loader::execute(&installed, args)
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let dir = self.plugins_root.join(name);
        if !dir.exists() {
            anyhow::bail!("plugin '{}' not installed", name);
        }
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
        Ok(())
    }
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        if file_name == ".odin-installed.json" {
            continue;
        }
        let dst_path = dst.join(&file_name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
