use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::integrations::process;
use crate::models::machine::PackageManagerInfo;
use crate::models::package::{InstalledPackage, PackageManager, PackageSnapshot};

pub async fn detect_managers() -> Vec<PackageManagerInfo> {
    let checks = [
        ("winget", executable("winget")),
        ("choco", choco_executable()),
        ("scoop", scoop_executable()),
        ("npm", executable("npm")),
        ("pipx", executable("pipx")),
        ("pnpm", executable("pnpm")),
        ("yarn", executable("yarn")),
        ("dotnet", executable("dotnet")),
        ("go", executable("go")),
        ("pip", executable("pip")),
        ("cargo", executable("cargo")),
        ("uv", executable("uv")),
    ];
    // Version probes shell out once per manager — run them concurrently.
    // Spawned handles are awaited in declaration order, so output order is
    // stable.
    let handles: Vec<_> = checks
        .into_iter()
        .map(|(name, executable)| {
            tokio::spawn(async move {
                let installed = executable.is_some();
                let version = if installed {
                    process::capture(executable.as_deref().unwrap_or(name), &["--version"])
                        .await
                        .ok()
                        .map(|out| out.stdout)
                        .filter(|s| !s.is_empty())
                } else {
                    None
                };
                PackageManagerInfo {
                    name: name.to_string(),
                    installed,
                    executable,
                    version,
                }
            })
        })
        .collect();
    let mut managers = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(info) => managers.push(info),
            Err(error) => eprintln!("warning: manager probe task failed: {error:#}"),
        }
    }
    managers
}

pub async fn list_packages() -> Result<PackageSnapshot> {
    // Each probe shells out to a different manager; running them sequentially
    // made snapshot latency the sum of ~12 subprocess round-trips. Spawn them
    // all and collect in a fixed order (ordering doesn't matter for the
    // result — it's sorted below — but deterministic collection keeps warning
    // output stable).
    let handles = [
        tokio::spawn(list_winget()),
        tokio::spawn(list_choco()),
        tokio::spawn(list_scoop()),
        tokio::spawn(list_npm_globals()),
        tokio::spawn(list_pip_globals()),
        tokio::spawn(list_cargo_installs()),
        tokio::spawn(list_pipx()),
        tokio::spawn(list_pnpm_globals()),
        tokio::spawn(list_yarn_globals()),
        tokio::spawn(list_dotnet_tools()),
        tokio::spawn(list_go_installs()),
        tokio::spawn(list_uv_tools()),
    ];
    let mut packages = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(mut manager_packages)) => packages.append(&mut manager_packages),
            Ok(Err(error)) => eprintln!("warning: package manager probe failed: {error:#}"),
            Err(error) => eprintln!("warning: package manager probe task failed: {error:#}"),
        }
    }
    packages.sort_by(|left, right| left.id.cmp(&right.id));
    packages.dedup_by(|left, right| {
        left.id.eq_ignore_ascii_case(&right.id) && left.source == right.source
    });
    Ok(PackageSnapshot { packages })
}

/// Canonical winget install command for a package id. `--source winget` pins
/// the query to the winget catalog only — without it winget also queries
/// msstore, which on machines where that source is broken (some Windows
/// Server AMIs) fails the whole lookup and demands interactive --source
/// disambiguation even though --id + --exact already pin the package.
/// Shared by capture AND restore so old snapshots replay the fixed form.
pub(crate) fn winget_install_command(id: &str) -> String {
    format!("winget install --id {id} --exact --source winget --accept-package-agreements --accept-source-agreements")
}

async fn list_winget() -> Result<Vec<InstalledPackage>> {
    let Some(winget) = executable("winget") else {
        return Ok(Vec::new());
    };
    let temp_path = std::env::temp_dir().join(format!("odin-winget-{}.json", uuid::Uuid::new_v4()));
    let temp_file = temp_path.to_string_lossy().to_string();
    let output = process::capture(
        &winget,
        &[
            "export",
            "-o",
            &temp_file,
            "--include-versions",
            "--accept-source-agreements",
        ],
    )
    .await?;
    if output.code != 0 || !temp_path.exists() {
        return Ok(Vec::new());
    }
    let data = tokio::fs::read_to_string(&temp_path)
        .await
        .unwrap_or_default();
    let _ = tokio::fs::remove_file(&temp_path).await;
    let json: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
    let packages = json
        .get("Sources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|source| {
            source
                .get("Packages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|pkg| {
            let id = pkg.get("PackageIdentifier")?.as_str()?.to_string();
            let version = pkg
                .get("Version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            Some(InstalledPackage {
                name: id.clone(),
                install_command: Some(winget_install_command(&id)),
                id,
                version,
                source: PackageManager::Winget,
            })
        })
        .collect();
    Ok(packages)
}

async fn list_choco() -> Result<Vec<InstalledPackage>> {
    let Some(choco) = choco_executable() else {
        return Ok(Vec::new());
    };
    let output = process::capture(&choco, &["list", "--local-only", "--limit-output"]).await?;
    if output.code != 0 {
        return Ok(Vec::new());
    }
    Ok(output
        .stdout
        .lines()
        .filter_map(|line| {
            let (id, version) = line.split_once('|')?;
            Some(InstalledPackage {
                id: id.to_string(),
                name: id.to_string(),
                version: Some(version.to_string()),
                source: PackageManager::Chocolatey,
                install_command: Some(format!("choco install {id} -y")),
            })
        })
        .collect())
}

async fn list_scoop() -> Result<Vec<InstalledPackage>> {
    let Some(scoop) = scoop_executable() else {
        return Ok(Vec::new());
    };
    let output = process::capture(&scoop, &["export"]).await?;
    if output.code != 0 {
        return Ok(Vec::new());
    }
    let json: Value = serde_json::from_str(&output.stdout).unwrap_or(Value::Null);
    let packages = json
        .get("apps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|app| {
            let name = app
                .get("Name")
                .or_else(|| app.get("name"))?
                .as_str()?
                .to_string();
            let version = app
                .get("Version")
                .or_else(|| app.get("version"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            Some(InstalledPackage {
                id: name.clone(),
                name: name.clone(),
                version,
                source: PackageManager::Scoop,
                install_command: Some(format!("scoop install {name}")),
            })
        })
        .collect();
    Ok(packages)
}

async fn list_npm_globals() -> Result<Vec<InstalledPackage>> {
    let Some(npm) = executable("npm") else {
        return Ok(Vec::new());
    };
    let output = process::capture(&npm, &["list", "-g", "--depth=0", "--json"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    let json: Value = serde_json::from_str(&output.stdout).unwrap_or(Value::Null);
    let deps = json
        .get("dependencies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Ok(deps
        .into_iter()
        .map(|(name, info)| {
            let version = info
                .get("version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            InstalledPackage {
                id: name.clone(),
                name: name.clone(),
                version,
                source: PackageManager::Npm,
                install_command: Some(format!("npm install -g {name}")),
            }
        })
        .collect())
}

async fn list_pip_globals() -> Result<Vec<InstalledPackage>> {
    // Try pip3 first, fall back to pip.
    let pip = if process::command_exists("pip3") {
        "pip3"
    } else if process::command_exists("pip") {
        "pip"
    } else {
        return Ok(Vec::new());
    };
    let output = process::capture(pip, &["list", "--format=json"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    let json: Value = serde_json::from_str(&output.stdout).unwrap_or(Value::Null);
    Ok(json
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|pkg| {
            let name = pkg.get("name")?.as_str()?.to_string();
            let version = pkg
                .get("version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            Some(InstalledPackage {
                id: name.clone(),
                name: name.clone(),
                version,
                source: PackageManager::Pip,
                install_command: Some(format!("pip install {name}")),
            })
        })
        .collect())
}

async fn list_cargo_installs() -> Result<Vec<InstalledPackage>> {
    let Some(cargo) = executable("cargo") else {
        return Ok(Vec::new());
    };
    let output = process::capture(&cargo, &["install", "--list"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    // Output format:
    //   ripgrep v13.0.0:
    //       rg
    //   cargo-edit v0.12.2:
    //       cargo-add
    //       cargo-rm
    // Lines ending with ':' are package headers; indented lines are binaries.
    let mut packages = Vec::new();
    for line in output.stdout.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            continue; // binary name line — skip
        }
        let header = line.trim_end_matches(':');
        // Split "name vX.Y.Z" — version token starts with 'v' followed by a digit.
        if let Some((name, version_raw)) = header.rsplit_once(' ') {
            let version = version_raw.trim_start_matches('v').to_string();
            packages.push(InstalledPackage {
                id: name.to_string(),
                name: name.to_string(),
                version: Some(version),
                source: PackageManager::Cargo,
                install_command: Some(format!("cargo install {name}")),
            });
        }
    }
    Ok(packages)
}

async fn list_pipx() -> Result<Vec<InstalledPackage>> {
    if !process::command_exists("pipx") {
        return Ok(Vec::new());
    }
    let output = process::capture("pipx", &["list", "--json"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    Ok(parse_pipx(&output.stdout))
}

fn parse_pipx(stdout: &str) -> Vec<InstalledPackage> {
    let json: Value = serde_json::from_str(stdout).unwrap_or(Value::Null);
    json.get("venvs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(_venv, entry)| {
            let main = entry.get("metadata")?.get("main_package")?;
            let name = main.get("package")?.as_str()?.to_string();
            let version = main
                .get("package_version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            Some(InstalledPackage {
                id: name.clone(),
                name: name.clone(),
                version,
                source: PackageManager::Pipx,
                install_command: Some(format!("pipx install {name}")),
            })
        })
        .collect()
}

async fn list_pnpm_globals() -> Result<Vec<InstalledPackage>> {
    if !process::command_exists("pnpm") {
        return Ok(Vec::new());
    }
    let output = process::capture("pnpm", &["ls", "-g", "--depth=0", "--json"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    Ok(parse_pnpm(&output.stdout))
}

fn parse_pnpm(stdout: &str) -> Vec<InstalledPackage> {
    // `pnpm ls -g --json` returns an array of project objects; the global root
    // carries the `dependencies` map.
    let json: Value = serde_json::from_str(stdout).unwrap_or(Value::Null);
    let projects = match &json {
        Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    };
    let mut packages = Vec::new();
    for project in projects {
        if let Some(deps) = project.get("dependencies").and_then(Value::as_object) {
            for (name, info) in deps {
                let version = info
                    .get("version")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                packages.push(InstalledPackage {
                    id: name.clone(),
                    name: name.clone(),
                    version,
                    source: PackageManager::Pnpm,
                    install_command: Some(format!("pnpm add -g {name}")),
                });
            }
        }
    }
    packages
}

async fn list_yarn_globals() -> Result<Vec<InstalledPackage>> {
    if !process::command_exists("yarn") {
        return Ok(Vec::new());
    }
    // Yarn Classic (v1) supports `global list`; Berry (v2+) removed it and prints
    // a usage error — parse_yarn returns nothing in that case, so we skip
    // gracefully.
    let output = process::capture("yarn", &["global", "list"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    Ok(parse_yarn(&output.stdout))
}

fn parse_yarn(stdout: &str) -> Vec<InstalledPackage> {
    // Lines look like: info "esbuild@0.19.0" has binaries:
    let mut packages = Vec::new();
    for line in stdout.lines() {
        let Some(start) = line.find('"') else {
            continue;
        };
        let rest = &line[start + 1..];
        let Some(end) = rest.find('"') else { continue };
        let spec = &rest[..end];
        // Split "name@version" from the right so scoped names (@scope/pkg) work.
        if let Some((name, version)) = spec.rsplit_once('@') {
            if name.is_empty() {
                continue;
            }
            packages.push(InstalledPackage {
                id: name.to_string(),
                name: name.to_string(),
                version: Some(version.to_string()),
                source: PackageManager::Yarn,
                install_command: Some(format!("yarn global add {name}")),
            });
        }
    }
    packages
}

async fn list_dotnet_tools() -> Result<Vec<InstalledPackage>> {
    if !process::command_exists("dotnet") {
        return Ok(Vec::new());
    }
    let output = process::capture("dotnet", &["tool", "list", "--global"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    Ok(parse_dotnet_tools(&output.stdout))
}

fn parse_dotnet_tools(stdout: &str) -> Vec<InstalledPackage> {
    // Fixed-width table:
    //   Package Id      Version      Commands
    //   -------------------------------------
    //   dotnetsay       2.1.4        dotnetsay
    let mut packages = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("---") || trimmed.starts_with("Package Id") {
            continue;
        }
        let mut cols = trimmed.split_whitespace();
        let (Some(id), Some(version)) = (cols.next(), cols.next()) else {
            continue;
        };
        packages.push(InstalledPackage {
            id: id.to_string(),
            name: id.to_string(),
            version: Some(version.to_string()),
            source: PackageManager::DotnetTool,
            install_command: Some(format!(
                "dotnet tool install --global {id} --version {version}"
            )),
        });
    }
    packages
}

async fn list_go_installs() -> Result<Vec<InstalledPackage>> {
    if !process::command_exists("go") {
        return Ok(Vec::new());
    }
    // Go has no "list installed binaries" command, so enumerate the bin dir
    // (GOBIN, else GOPATH/bin, else ~/go/bin) and ask `go version -m` for each
    // binary's originating module. Best-effort: any failure skips that entry.
    let Some(bin_dir) = go_bin_dir() else {
        return Ok(Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(&bin_dir) else {
        return Ok(Vec::new());
    };

    let mut packages = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        // On Windows executables are .exe; elsewhere no extension.
        if cfg!(windows) && !ext.eq_ignore_ascii_case("exe") {
            continue;
        }
        let file = path.to_string_lossy().to_string();
        let Ok(output) = process::capture("go", &["version", "-m", &file]).await else {
            continue;
        };
        if let Some(pkg) = parse_go_version_m(&output.stdout) {
            packages.push(pkg);
        }
    }
    Ok(packages)
}

fn go_bin_dir() -> Option<PathBuf> {
    if let Ok(gobin) = std::env::var("GOBIN") {
        if !gobin.is_empty() {
            return Some(PathBuf::from(gobin));
        }
    }
    if let Ok(gopath) = std::env::var("GOPATH") {
        if let Some(first) = std::env::split_paths(&gopath).next() {
            return Some(first.join("bin"));
        }
    }
    std::env::var("USERPROFILE")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(|home| Path::new(&home).join("go").join("bin"))
}

fn parse_go_version_m(output: &str) -> Option<InstalledPackage> {
    // `go version -m <file>` prints tab-indented lines:
    //         path    github.com/user/tool/cmd/tool
    //         mod     github.com/user/tool    v1.2.3    h1:...
    let mut pkg_path: Option<String> = None;
    let mut version: Option<String> = None;
    for line in output.lines() {
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("path") => pkg_path = fields.next().map(ToOwned::to_owned),
            Some("mod") => {
                let _module = fields.next();
                version = fields.next().map(ToOwned::to_owned);
            }
            _ => {}
        }
    }
    let path = pkg_path?;
    let ver = version.unwrap_or_else(|| "latest".to_string());
    Some(InstalledPackage {
        id: path.clone(),
        name: path.rsplit('/').next().unwrap_or(&path).to_string(),
        version: Some(ver.clone()),
        source: PackageManager::Go,
        install_command: Some(format!("go install {path}@{ver}")),
    })
}

async fn list_uv_tools() -> Result<Vec<InstalledPackage>> {
    if !process::command_exists("uv") {
        return Ok(Vec::new());
    }
    let output = process::capture("uv", &["tool", "list"]).await?;
    if output.code != 0 || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    Ok(parse_uv_tools(&output.stdout))
}

fn parse_uv_tools(stdout: &str) -> Vec<InstalledPackage> {
    // Output:
    //   black v23.1.0
    //   - black
    //   ruff v0.1.0
    //   - ruff
    let mut packages = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        // Binary lines start with "-"; tool header lines are "name vX.Y.Z".
        if trimmed.is_empty() || trimmed.starts_with('-') {
            continue;
        }
        let Some((name, version_raw)) = trimmed.split_once(' ') else {
            continue;
        };
        let version = version_raw.trim().trim_start_matches('v').to_string();
        packages.push(InstalledPackage {
            id: name.to_string(),
            name: name.to_string(),
            version: Some(version),
            source: PackageManager::Uv,
            install_command: Some(format!("uv tool install {name}")),
        });
    }
    packages
}

fn executable(name: &str) -> Option<String> {
    if process::command_exists(name) {
        Some(name.to_string())
    } else {
        None
    }
}

pub(crate) fn scoop_executable() -> Option<String> {
    if process::command_exists("scoop") {
        return Some("scoop".to_string());
    }

    for candidate in scoop_candidates() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

pub(crate) fn choco_executable() -> Option<String> {
    if process::command_exists("choco") {
        return Some("choco".to_string());
    }

    for candidate in choco_candidates() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

pub(crate) fn choco_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(chocolatey_install) = std::env::var("ChocolateyInstall") {
        candidates.push(Path::new(&chocolatey_install).join(r"bin\choco.exe"));
        candidates.push(Path::new(&chocolatey_install).join(r"bin\choco.bat"));
    }
    if let Ok(program_data) = std::env::var("ProgramData") {
        candidates.push(Path::new(&program_data).join(r"chocolatey\bin\choco.exe"));
        candidates.push(Path::new(&program_data).join(r"chocolatey\bin\choco.bat"));
    }
    candidates
}

pub(crate) fn scoop_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        candidates.push(Path::new(&user_profile).join(r"scoop\shims\scoop.cmd"));
        candidates.push(Path::new(&user_profile).join(r"scoop\shims\scoop.ps1"));
    }
    if let Ok(scoop) = std::env::var("SCOOP") {
        candidates.push(Path::new(&scoop).join(r"shims\scoop.cmd"));
        candidates.push(Path::new(&scoop).join(r"shims\scoop.ps1"));
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pipx_json() {
        let json = r#"{
          "venvs": {
            "black": { "metadata": { "main_package": { "package": "black", "package_version": "23.1.0" } } },
            "poetry": { "metadata": { "main_package": { "package": "poetry", "package_version": "1.7.0" } } }
          }
        }"#;
        let pkgs = parse_pipx(json);
        assert_eq!(pkgs.len(), 2);
        let black = pkgs.iter().find(|p| p.id == "black").unwrap();
        assert_eq!(black.version.as_deref(), Some("23.1.0"));
        assert_eq!(black.source, PackageManager::Pipx);
        assert_eq!(black.install_command.as_deref(), Some("pipx install black"));
    }

    #[test]
    fn parses_pnpm_json_array() {
        let json = r#"[{ "dependencies": { "typescript": { "version": "5.4.0" }, "eslint": { "version": "9.0.0" } } }]"#;
        let pkgs = parse_pnpm(json);
        assert_eq!(pkgs.len(), 2);
        let ts = pkgs.iter().find(|p| p.id == "typescript").unwrap();
        assert_eq!(ts.version.as_deref(), Some("5.4.0"));
        assert_eq!(
            ts.install_command.as_deref(),
            Some("pnpm add -g typescript")
        );
    }

    #[test]
    fn parses_yarn_classic_text() {
        let text = "yarn global v1.22.19\ninfo \"esbuild@0.19.0\" has binaries:\n   - esbuild\ninfo \"@angular/cli@17.0.0\" has binaries:\n   - ng\n";
        let pkgs = parse_yarn(text);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].id, "esbuild");
        assert_eq!(pkgs[0].version.as_deref(), Some("0.19.0"));
        // Scoped package name preserved.
        assert_eq!(pkgs[1].id, "@angular/cli");
        assert_eq!(pkgs[1].version.as_deref(), Some("17.0.0"));
    }

    #[test]
    fn parses_dotnet_tool_table() {
        let table = "Package Id      Version      Commands\n----------------------------------------\ndotnetsay       2.1.4        dotnetsay\ncsharpier       0.27.0       csharpier\n";
        let pkgs = parse_dotnet_tools(table);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].id, "dotnetsay");
        assert_eq!(pkgs[0].version.as_deref(), Some("2.1.4"));
        assert_eq!(
            pkgs[0].install_command.as_deref(),
            Some("dotnet tool install --global dotnetsay --version 2.1.4")
        );
    }

    #[test]
    fn parses_uv_tool_list() {
        let text = "black v23.1.0\n- black\nruff v0.1.0\n- ruff\n";
        let pkgs = parse_uv_tools(text);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].id, "black");
        assert_eq!(pkgs[0].version.as_deref(), Some("23.1.0"));
        assert_eq!(pkgs[1].id, "ruff");
        assert_eq!(
            pkgs[1].install_command.as_deref(),
            Some("uv tool install ruff")
        );
    }

    #[test]
    fn parses_go_version_m() {
        let output = "C:\\Users\\ada\\go\\bin\\gopls.exe: go1.21.0\n\tpath\tgolang.org/x/tools/gopls\n\tmod\tgolang.org/x/tools/gopls\tv0.14.2\th1:abc\n";
        let pkg = parse_go_version_m(output).unwrap();
        assert_eq!(pkg.id, "golang.org/x/tools/gopls");
        assert_eq!(pkg.name, "gopls");
        assert_eq!(pkg.version.as_deref(), Some("v0.14.2"));
        assert_eq!(
            pkg.install_command.as_deref(),
            Some("go install golang.org/x/tools/gopls@v0.14.2")
        );
    }

    #[test]
    fn yarn_berry_usage_error_yields_nothing() {
        // Berry has no `global` command; its error text has no quoted spec lines.
        let text = "Usage Error: Couldn't find the node_modules state file\n";
        assert!(parse_yarn(text).is_empty());
    }
}
