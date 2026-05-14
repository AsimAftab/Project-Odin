use anyhow::{Context, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use std::fs::File;
use std::path::Path;

pub fn create_tarball(input_dir: &Path, output: &Path) -> Result<()> {
    if !input_dir.is_dir() {
        anyhow::bail!("not a directory: {}", input_dir.display());
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let file =
        File::create(output).with_context(|| format!("creating archive {}", output.display()))?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder
        .append_dir_all(".", input_dir)
        .with_context(|| format!("appending {} to archive", input_dir.display()))?;
    builder.finish().context("finishing tar archive")?;
    Ok(())
}

pub fn extract_tarball(input: &Path, output_dir: &Path) -> Result<()> {
    if !input.is_file() {
        anyhow::bail!("not a file: {}", input.display());
    }
    std::fs::create_dir_all(output_dir)?;
    let file = File::open(input).with_context(|| format!("opening archive {}", input.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(output_dir)
        .with_context(|| format!("extracting to {}", output_dir.display()))?;
    Ok(())
}
