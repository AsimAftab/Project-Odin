# Odin

Odin is a Windows-first developer environment management CLI. It snapshots the tools, package managers, shell profiles, Git settings, VS Code extensions, terminal settings, environment variables, and PATH state needed to recreate a workstation.

The primary workflow is:

```powershell
odin snapshot
odin sync --remote https://github.com/you/private-odin-state.git
odin restore --apply
```

## Commands

### Environment & Configuration
```text
odin snapshot   Capture current machine state into ~/.odin
odin restore    Reinstall and restore from the latest snapshot
odin diff       Compare the live machine against the last snapshot
odin export     Generate PowerShell bootstrap and restore scripts
odin init       Initialize ~/.odin/config.yaml
odin config     Configure GitHub and show local configuration
```

### Monitoring & Diagnostics
```text
odin all-eye    Hliðskjálf — interactive overview (alias: `odin dashboard`)
odin doctor     Diagnose broken PATH entries, missing SDKs, and conflicts
odin ports      List all listening ports with process information (JSON support)
odin ps         Interactive process dashboard (htop-style with live metrics)
odin freeport   Free a bound port or PID with safety checks (alias: `odin kill`)
```

### Time Machine & History
```text
odin history    View snapshot history with colored timeline
odin rollback   Restore environment to a previous snapshot
odin schedule   Register/remove a recurring snapshot task (Windows Task Scheduler)
```

### Sync & Updates
```text
odin sync       Commit and push snapshots to a GitHub repository
odin backup     Alias for `odin sync` (online backup to Git)
odin update     Check for and install the latest Odin release
```

### Asgard — Developer Profile Realm
```text
odin asgard                Enter the profile realm — interactive TUI selector
odin activate <profile>    Bind a realm by name (non-interactive)
odin deactivate            Unbind the active realm
odin current               Show the bound realm and recent bindings
odin profile list          List realms (supports --json)
odin profile create        Forge a new realm via the themed wizard
odin profile edit <name>   Edit a realm interactively
odin profile delete <name> Dissolve a realm
odin profile export <name> Bundle a realm to a .tar.gz
odin profile import <path> Restore a realm from a .tar.gz
```

Asgard is the realm where named workstation profiles live. A realm bundles environment variables (runes), startup apps (warriors), browser URLs (ravens), and an optional VS Code workspace; binding it launches everything in one shot.

The interactive selector (`odin asgard`) draws the world tree Yggdrasil at the crown, lists every realm with rich detail in a side scroll, and shows recent bindings — keys: `↑↓` navigate, `Enter` bind, `N` new, `E` edit, `D` delete, `X` unbind, `?` help, `Q` leave.

Profiles live under `%USERPROFILE%\.odin\asgard\<name>\profile.yaml`. See `examples/profiles/` for templates.

### Glossary — Odin's lexicon

Odin's commands and TUIs use a Norse-mythology vocabulary. Quick translation:

| Theme term | Concrete meaning |
| --- | --- |
| Realm | A profile / a workstation captured by a snapshot |
| Vault | The `~/.odin` directory holding snapshots |
| Rune | A snapshot ID (UUID) or environment variable |
| Bind / Unbind | Activate / deactivate (a profile, or a snapshot via restore) |
| Forge | Package manager · or, the act of creating |
| Warrior | A startup app that rides out on activation |
| Raven | A browser URL sent flying on activation |
| Bifrost | GitHub sync (the rainbow bridge) |
| Hliðskjálf | The All-Eye dashboard (Odin's high seat) |
| Yggdrasil | The world tree painted in the Asgard banner |
| Hugin & Munin | Health checks · also, `odin watch` |
| Mjölnir | The hammer that frees a port (`odin freeport`) |

```yaml
name: backend-dev
description: Backend Rust + Postgres workflow
env:
  RUST_LOG: debug
startup_apps:
  - name: editor
    command: code
    args: ["."]
    cwd: C:\repos\backend
    window: maximized
  - name: terminal
    command: wt.exe
    args: [new-tab, -p, PowerShell]
vscode_workspace: C:\repos\backend\backend.code-workspace
browser_urls:
  - https://github.com/myorg/backend/pulls
```

`window` accepts `normal`, `minimized`, or `maximized`. `command` may be an exe, a command on PATH, a `.lnk`, or a URL (URLs open in the default browser). Env vars are applied per spawned process; deactivating clears the active marker but does not kill running apps.

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

Configure and push in one step:

```powershell
odin config github --sync-now
```

The GitHub token is stored in the OS credential store.

Odin can also create a private GitHub repository through the GitHub API:

```powershell
$env:GITHUB_TOKEN = "ghp_..."
odin sync --create-private-repo --github-repo odin-state
```

## Time Machine (Phase 2)

Odin tracks all snapshots and enables rolling back to previous states:

```powershell
# View the history of snapshots
odin history

# View detailed changes between snapshots
odin history --detailed

# Rollback to a previous snapshot (dry-run by default)
odin rollback snapshot-20250313-120000

# Apply the rollback
odin rollback snapshot-20250313-120000 --apply
```

This enables powerful workflows like:
- Quickly reverting environment changes
- Comparing machine state across time periods
- Auditing which packages were added/removed
- Testing environment configurations before applying

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

`odin all-eye` opens a Ratatui terminal dashboard when running in an interactive terminal. It shows snapshot metadata, developer tools, package managers, GitHub sync state, and health indicators. Press `q` or `Esc` to quit. (`odin dashboard` is kept as an alias.)

## Install

### winget (recommended)

```powershell
winget install AsimAftab.Odin
```

`odin` is on PATH immediately, no SmartScreen warning, automatic updates via `winget upgrade`, and clean uninstall via `winget uninstall AsimAftab.Odin`. The workspace at `%USERPROFILE%\.odin` is created automatically on first use; run `odin init` only if you want the interactive setup with PATH validation and dependency checks.

### PowerShell (CI/automation)

Bootstrap from GitHub Releases:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\bootstrap.ps1 -Repository OWNER/REPO -Scope User
```

System-wide install (admin shell):

```powershell
.\scripts\install.ps1 -Scope Machine -Repository OWNER/REPO
```

Uninstall via script:

```powershell
.\scripts\uninstall.ps1 -Scope User
```

### Build from source

```powershell
cargo build --release
.\scripts\install.ps1 -LocalBinary .\target\release\odin.exe -Force
odin --help
```

To build the MSI locally (requires the [WiX Toolset 3.x](https://wixtoolset.org/docs/wix3/) installed):

```powershell
cargo install cargo-wix --locked
cargo wix --nocapture
# MSI lands in target\wix\odin-<version>-x86_64.msi
```

## Release Automation

GitHub Actions workflows are included:

- `.github/workflows/ci.yml` runs format checks, clippy, tests, and release build.
- `.github/workflows/release.yml` builds `odin.exe` and the MSI installer, creates a GitHub Release, and uploads:
  - `odin-<version>-x86_64.msi` (MSI installer)
  - `odin.exe`
  - `odin-windows-x64.zip`
  - `install.ps1`, `uninstall.ps1`, `bootstrap.ps1`
  - `checksums.txt`

## Development

```powershell
cargo fmt
cargo test
cargo run -- snapshot
cargo run -- update --check
```

The codebase is organized around command handlers, services, typed models, utility code, and Windows-specific integrations so Linux/macOS support and future plugin/Sentinel AI integrations can be added behind stable service interfaces.

## Key Features

✨ **Real-time Process Monitoring**: Interactive process dashboard with sorting, filtering, and resource monitoring
🔌 **Port Management**: List listening ports, identify processes, and kill them safely
⚡ **Auto-Update**: Check and install latest releases from GitHub with one command
💾 **Snapshot & Restore**: Capture and restore complete developer environment
🔄 **GitHub Sync**: Backup configuration to private GitHub repositories
🏥 **Diagnostics**: Health checks for PATH, SDKs, package managers, and VS Code
🎨 **Beautiful CLI**: Colorful ASCII art banner with command guidance

## Docs

- [Features Guide](docs/features.md) - Complete feature documentation with examples
- [Usage](docs/usage.md) - Common workflows and commands
- [Architecture](docs/architecture.md) - Code organization and design
- [GitHub Sync](docs/github.md) - Private repository backup setup
- [Release Process](docs/release.md) - Building and publishing releases
