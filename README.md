# Odin

Odin is a Windows-first developer environment management CLI. It snapshots the tools, package managers, shell profiles, Git settings, VS Code extensions, terminal settings, environment variables, and PATH state needed to recreate a workstation.

The primary workflow is:

```powershell
odin snapshot
odin sync --remote https://github.com/you/private-odin-state.git
odin restore --apply
```

## Commands

```text
odin snapshot   Capture current machine state into ~/.odin
odin dashboard  Show snapshot status and next commands
odin restore    Reinstall and restore from the latest snapshot
odin sync       Commit and push snapshots to a GitHub repository
odin doctor     Diagnose broken PATH entries, missing SDKs, and conflicts
odin diff       Compare the live machine against the last snapshot
odin export     Generate PowerShell bootstrap and restore scripts
odin init       Initialize ~/.odin/config.yaml
odin config     Configure GitHub and show local configuration
```

## Snapshot Output

By default Odin writes to `%USERPROFILE%\.odin`:

```text
machine.json
env.json
packages.json
vscode_extensions.json
git_config.json
restore.ps1
install.ps1
bootstrap.ps1
odin.lock
```

Snapshots are plain JSON so they can be reviewed, committed, and restored without a proprietary backend.

## Restore Safety

`odin restore` runs in dry-run mode by default. Use `--apply` to execute install and restore operations:

```powershell
odin restore --apply
```

The restore engine skips packages that already appear installed and logs each package-manager command before execution.

## GitHub Sync

`odin sync` initializes `%USERPROFILE%\.odin` as a Git repository when needed, commits changed snapshot files, and pushes to the configured remote.

```powershell
odin sync --remote https://github.com/you/private-odin-state.git --branch main
```

For private repositories, use normal Git credential helpers or configure Odin:

```powershell
odin config github
odin sync
```

The GitHub token is stored in the OS credential store.

Odin can also create a private GitHub repository through the GitHub API:

```powershell
$env:GITHUB_TOKEN = "ghp_..."
odin sync --create-private-repo --github-repo odin-state
```

## Windows Integrations

Current integrations include:

- winget
- Chocolatey
- Scoop
- PowerShell profile discovery
- Git config discovery and restore
- VS Code extension discovery and restore
- Windows Terminal settings discovery
- environment variables and PATH analysis

## Configuration

Odin stores local configuration in `%USERPROFILE%\.odin\config.yaml`.
An example configuration is available in [examples/odin.yaml](examples/odin.yaml).

```powershell
odin init
odin config show
```

## Dashboard

`odin dashboard` opens a Ratatui terminal dashboard when running in an interactive terminal. It shows snapshot metadata, developer tools, package managers, GitHub sync state, and health indicators. Press `q` or `Esc` to quit.

## Install Locally

```powershell
cargo build --release
.\scripts\install.ps1
odin --help
```

## Release Automation

GitHub Actions workflows are included:

- `.github/workflows/ci.yml` runs format checks, clippy, tests, and release build.
- `.github/workflows/release.yml` builds `odin.exe`, packages it, creates a GitHub Release, and uploads assets on `v*.*.*` tags.

## Development

```powershell
cargo fmt
cargo test
cargo run -- snapshot
```

The codebase is organized around command handlers, services, typed models, utility code, and Windows-specific integrations so Linux/macOS support and future plugin/Sentinel AI integrations can be added behind stable service interfaces.

## Docs

- [Usage](docs/usage.md)
- [Architecture](docs/architecture.md)
- [GitHub Sync](docs/github.md)
- [Release Process](docs/release.md)
