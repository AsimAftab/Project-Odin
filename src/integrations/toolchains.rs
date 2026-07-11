//! Language-toolchain version capture (rustup toolchains, node version
//! managers, pythons). Capture-only for now: the data rides along in
//! machine.json so a rebuilt workstation knows which toolchain versions to
//! reinstall, even before Odin can restore them itself.

use crate::integrations::process;
use crate::models::machine::ToolchainInfo;

/// Cap per manager so a pathological probe can't bloat the snapshot.
const MAX_ITEMS: usize = 50;

pub async fn capture() -> Vec<ToolchainInfo> {
    let probes: [(&str, &str, &[&str]); 5] = [
        ("rustup", "rustup", &["toolchain", "list"]),
        ("volta", "volta", &["list", "--format", "plain"]),
        ("nvm", "nvm", &["list"]),
        ("pyenv", "pyenv", &["versions", "--bare"]),
        ("uv", "uv", &["python", "list", "--only-installed"]),
    ];

    let handles: Vec<_> = probes
        .into_iter()
        .map(|(manager, exe, args)| {
            tokio::spawn(async move {
                if !process::command_exists(exe) {
                    return None;
                }
                let output = process::capture(exe, args).await.ok()?;
                if output.code != 0 {
                    return None;
                }
                let items = parse_lines(&output.stdout);
                (!items.is_empty()).then(|| ToolchainInfo {
                    manager: manager.to_string(),
                    items,
                })
            })
        })
        .collect();

    let mut toolchains = Vec::new();
    for handle in handles {
        if let Ok(Some(info)) = handle.await {
            toolchains.push(info);
        }
    }
    toolchains
}

/// Non-empty trimmed lines, capped. Kept as raw manager output lines — each
/// manager has its own format and the value is in fidelity, not parsing.
fn parse_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(MAX_ITEMS)
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lines_trims_filters_and_caps() {
        let out = "  stable-x86_64-pc-windows-msvc (default)  \n\n  nightly\n";
        assert_eq!(
            parse_lines(out),
            vec!["stable-x86_64-pc-windows-msvc (default)", "nightly"]
        );

        let many = (0..100).map(|i| format!("v{i}\n")).collect::<String>();
        assert_eq!(parse_lines(&many).len(), MAX_ITEMS);
    }
}
