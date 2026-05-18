# Odin Features Guide

Comprehensive reference for all Odin commands and capabilities.

## Table of Contents

1. [Snapshot & Restore](#snapshot--restore)
2. [Process & Port Management](#process--port-management)
3. [Diagnostics](#diagnostics)
4. [Sync & Backup](#sync--backup)
5. [Updates](#updates)
6. [Time Machine & History](#time-machine--history-phase-2)
7. [Configuration](#configuration)
8. [Dashboard](#dashboard)
9. [Command Reference](#command-reference)

---

## Snapshot & Restore

### `odin snapshot`

Captures the current machine state into `~/.odin` directory.

**What gets captured:**
- **Machine info**: OS version, hostname, CPU, memory, disk
- **Environment variables**: PATH, user env vars, system env vars
- **Installed packages**: winget, Chocolatey, Scoop packages (with versions)
- **Git configuration**: Git config (global + local if repos present)
- **VS Code extensions**: All installed extensions with versions
- **Windows Terminal settings**: Terminal configuration and color schemes
- **PowerShell profiles**: Available profiles and locations

**Output files:**
```
~/.odin/
├── machine.json              # System hardware/OS info
├── env.json                  # Environment variables and PATH
├── packages.json             # Installed packages from all managers
├── vscode_extensions.json    # VS Code extensions list
├── git_config.json           # Git configuration
├── odin.lock                 # Snapshot timestamp and metadata
├── restore.ps1               # Auto-generated restore script
├── install.ps1               # Auto-generated install script
└── bootstrap.ps1             # Auto-generated bootstrap script
```

**Usage:**
```powershell
# Simple snapshot
odin snapshot

# Output as JSON
odin snapshot --json
```

**Example output:**
```json
{
  "machine": {
    "os": "Windows 11 23H2",
    "hostname": "WORKSTATION-01",
    "cpus": 12,
    "memory_gb": 32
  },
  "packages": [
    { "manager": "winget", "name": "Microsoft.VisualStudioCode", "version": "1.92.0" },
    { "manager": "winget", "name": "Git.Git", "version": "2.45.0" }
  ]
}
```

---

### `odin restore`

Reinstalls and restores environment from the latest snapshot.

**Features:**
- **Dry-run by default**: Preview all changes without applying
- **Smart install**: Skips packages already installed
- **Safe defaults**: Requires `--apply` to actually modify system
- **Package manager support**: winget, Chocolatey, Scoop restoration
- **Git config restore**: Restores git user, email, core settings
- **VS Code extension restore**: Installs missing extensions
- **Environment variable restore**: Sets PATH and user variables

**Usage:**
```powershell
# Preview restore (dry-run)
odin restore

# Apply restore changes
odin restore --apply

# Show as JSON
odin restore --json
```

**Output example:**
```
[SKIP] Package already installed: Microsoft.VisualStudioCode v1.92.0
[INSTALL] PowerShell 7.4.0 via winget
[INSTALL] VS Code extension ms-python.python
[CONFIG] Setting git user.name = "John Doe"
```

---

### `odin diff`

Compares live machine state against the last snapshot.

**Shows:**
- New packages installed since snapshot
- Removed packages
- Changed versions
- Environment variable differences
- New/removed VS Code extensions
- Git config changes

**Usage:**
```powershell
# Plain text diff
odin diff

# JSON diff
odin diff --json
```

**Example:**
```
PACKAGES:
  [NEW] Microsoft.PowerShell v7.4.0 (winget)
  [REMOVED] OldApp v1.0.0 (chocolatey)
  [VERSION] Python 3.10.5 → 3.11.0 (winget)

VSCODE EXTENSIONS:
  [NEW] ms-python.python v2024.0.0
  [REMOVED] GitHub.copilot-nightly
```

---

### `odin export`

Generates PowerShell bootstrap and restore scripts for sharing environments.

**Generated scripts:**
- `bootstrap.ps1` - Download and install Odin
- `install.ps1` - Setup PATH and install binary
- `restore.ps1` - Install all packages and restore config

**Usage:**
```powershell
odin export

# Generate with custom output directory
odin export --output ./scripts

# Show as JSON
odin export --json
```

---

## Process & Port Management

### `odin ports`

Lists all listening ports with associated process information.

**Shows:**
- Port number
- Protocol (TCP/UDP)
- Local address
- Process name and ID (PID)
- Process executable path

**Columns:**
```
PORT    PROTOCOL    ADDRESS           PROCESS              PID
3000    TCP         127.0.0.1:3000    node.exe            1234
5432    TCP         127.0.0.1:5432    postgres.exe        5678
8080    TCP         0.0.0.0:8080      dotnet.exe          2468
```

**Usage:**
```powershell
# List all listening ports
odin ports

# JSON output (for tooling)
odin ports --json

# Sort by port (default), process, or protocol
odin ports --sort port
odin ports --sort process
```

**Example JSON:**
```json
{
  "ports": [
    {
      "port": 3000,
      "protocol": "TCP",
      "address": "127.0.0.1:3000",
      "process_name": "node.exe",
      "pid": 1234,
      "executable": "C:\\Program Files\\nodejs\\node.exe"
    }
  ]
}
```

**Use cases:**
- Find which app is using a port
- Identify port conflicts
- Monitor development server ports
- Check active network services

---

### `odin ps`

Interactive process dashboard (similar to htop).

**Features:**
- **Live updates**: Real-time process list (500ms refresh)
- **Sorting**: Click columns or press 1-4 to sort by CPU, Memory, Threads, Name
- **Filtering**: Type to search for processes
- **Resource monitoring**: CPU %, Memory, Thread count, Status
- **Safe kill**: Press 'K' to kill selected process (requires confirmation)
- **Keyboard controls**: Arrow keys to navigate, Q to quit

**Display:**
```
┌─ Process Monitor ────────────────────────────────────────┐
│ Process Name        CPU%    Memory    Threads    Status   │
│ ─────────────────────────────────────────────────────────│
│ explorer.exe        0.5%    450 MB    120        Running  │
│ chrome.exe          12.3%   2.1 GB    180        Running  │
│ node.exe            45.2%   1.8 GB    50         Running  │
│ vscode.exe          8.1%    950 MB    80         Running  │
└─────────────────────────────────────────────────────────┘

Q: Quit | Arrow: Navigate | K: Kill | 1-4: Sort | /: Filter
```

**Usage:**
```powershell
# Open interactive dashboard
odin ps

# JSON snapshot of processes
odin ps --json
```

**Keyboard Controls:**
| Key | Action |
|-----|--------|
| `↑` `↓` | Navigate process list |
| `1` | Sort by Process Name |
| `2` | Sort by CPU % |
| `3` | Sort by Memory |
| `4` | Sort by Thread Count |
| `K` | Kill selected process (with confirmation) |
| `/` | Search/filter processes |
| `Q` / `Esc` | Quit dashboard |

**Use cases:**
- Monitor resource-heavy processes
- Quick process identification
- Kill stuck processes interactively
- Real-time performance analysis

---

### `odin freeport`

Free a port or PID by terminating the bound process. Aliased as `odin kill` for backwards compatibility.

**Features:**
- **Smart detection**: Automatically detects if input is port or PID
- **Safety first**: Requires `--force` flag to prevent accidents
- **Clear feedback**: Shows the bound process and a Mjölnir-themed success line
- **Port resolution**: Finds PID from listening port

**Port/PID Detection Logic:**
- Numbers 1024-65535 assumed to be ports
- Other positive integers treated as PIDs

**Usage:**
```powershell
# Free a port (port must be listening)
odin freeport 3000 --force

# Free by PID
odin freeport 1234 --force

# Preview (without --force) - shows what would be freed
odin freeport 3000
```

**Examples:**
```powershell
# Port-based free (common for dev servers)
odin freeport 8080 --force

# PID-based free
odin freeport 5678 --force

# Preview mode (no --force)
odin freeport 3000
```

**Safety Features:**
- Dry-run by default (no `--force`)
- Shows exact process being killed
- Clear error messages for invalid targets
- Prevents accidental kills with confirmation pattern

---

## Diagnostics

### `odin doctor`

Diagnoses machine health and identifies issues.

**Checks:**
- **PATH validation**: Detects broken paths and duplicates
- **SDK availability**: Python, Node.js, Rust, .NET, Java versions
- **Package managers**: winget, Chocolatey, Scoop availability
- **Git configuration**: git user, email, credential helpers
- **VS Code setup**: Installation and extension count
- **PowerShell profiles**: Profile paths and conflicts
- **Environment variables**: Unset required variables
- **Disk space**: Available space on system drive

**Output levels:**
- 🟢 **OK**: All good
- 🟡 **WARNING**: Potential issue
- 🔴 **ERROR**: Configuration problem

**Usage:**
```powershell
# Run diagnostics
odin doctor

# Show as JSON for parsing
odin doctor --json
```

**Example output:**
```
🏥 Odin Health Check
────────────────────────────────────────
System & Paths:
  🟢 Windows 11 23H2
  🟢 PowerShell 7.4.0
  🟡 PATH has duplicate entries (2 instances of C:\Program Files\Git\bin)
  
SDKs:
  🟢 Python 3.11.0 (C:\Python311\python.exe)
  🟢 Node.js 20.10.0 (C:\Program Files\nodejs\node.exe)
  🟢 Rust 1.75.0 (C:\Users\user\.cargo\bin)
  🔴 .NET SDK not found
  
Package Managers:
  🟢 winget 1.6.0
  🟢 Chocolatey 2.2.2
  🟡 Scoop not found
  
Git:
  🟢 Git 2.45.0
  🟢 User: john.doe
  🟢 Email: john.doe@example.com
  
VS Code:
  🟢 Installed: C:\Program Files\Microsoft VS Code\Code.exe
  🟢 Extensions: 45 installed
```

---

### `odin all-eye`

The All-Eye — Odin's gaze from Hliðskjálf. Interactive dashboard showing snapshot status, observers, and the Bifrost (sync) state. Aliased as `odin dashboard` for backwards compatibility.

**Displays:**
- Last snapshot time and age
- Machine info (CPU, RAM, OS)
- Package managers (forges) with ready/dormant state
- Developer tools (ravens)
- Hugin & Munin observers (health checks)
- Bifrost (GitHub sync) state
- Mead-hall gauge (forges ready ratio)

**Features:**
- **Interactive TUI**: Themed runic banner and status indicators
- **Status at a glance**: See system health and sync state
- **Plain-text fallback**: Non-TTY runs print a themed text overview

**Usage:**
```powershell
# Ascend to Hliðskjálf
odin all-eye

# Same thing, classic name
odin dashboard
```

**Keyboard Controls:**
| Key | Action |
|-----|--------|
| `S` | Run snapshot |
| `R` | Run restore (preview) |
| `A` | Apply restore |
| `B` | Backup to GitHub (sync) |
| `D` | Run doctor |
| `U` | Check updates |
| `Q` / `Esc` | Quit |

---

## Sync & Backup

### `odin sync` / `odin backup`

Commit and push snapshots to GitHub repository.

**Workflow:**
1. Creates `~/.odin` as git repository (if needed)
2. Commits changed snapshot files
3. Pushes to configured remote/branch
4. Updates GitHub sync status

**Features:**
- **Incremental commits**: Only commits changed snapshot files
- **Automatic initialization**: Sets up git repo if needed
- **Private repo support**: Works with private GitHub repositories
- **Credential storage**: Stores GitHub token securely in OS credential store
- **Auto-create repo**: Can create private GitHub repo automatically

**Configuration:**

```powershell
# Interactive GitHub setup
odin config github

# One-step: configure and sync
odin config github --sync-now

# Manual configuration
odin sync --remote https://github.com/you/odin-state.git --branch main
```

**Setup examples:**

```powershell
# Using GitHub token (stored securely)
$env:GITHUB_TOKEN = "ghp_xxxxxxxxxxxx"
odin config github

# Create private repo automatically
odin sync --create-private-repo --github-repo my-odin-state

# Specify custom branch
odin sync --branch development
```

**Credentials:**
- GitHub token stored in Windows Credential Manager
- Never exposed in environment or logs
- Can be cleared and reset anytime

**Usage:**
```powershell
# Preview sync
odin sync --dry-run

# Actually sync to GitHub
odin sync

# Custom branch
odin sync --branch staging

# Create private repo and sync
odin sync --create-private-repo
```

---

## Updates

### `odin update`

Check for and install the latest Odin release from GitHub.

**Features:**
- **Version detection**: Compares current version with latest release
- **Smart updates**: Only downloads if newer version available
- **Safe replacement**: Stages binary replacement (applies after process exit)
- **Rollback capable**: Previous version backed up
- **GitHub integration**: Reads releases from GitHub API

**Usage:**
```powershell
# Check if update available (no download)
odin update --check

# Install latest version
odin update

# Show as JSON
odin update --json
```

**Example output:**
```powershell
# Current version check
odin update --check
# Output: You are on version 0.1.0 (latest)

# If update available
odin update --check
# Output: New version 0.1.1 available! Run 'odin update' to install

# Apply update
odin update
# Output: 
# Downloading Odin v0.1.1...
# ✓ Downloaded odin.exe (3.2 MB)
# ✓ Verified checksum
# ✓ Staged replacement - will apply on next launch
```

**How it works:**
1. Fetches latest release from GitHub
2. Compares versions
3. Downloads binary if newer
4. Verifies checksum
5. Stages replacement in temp directory
6. Applies replacement after process exits

**Configuration:**
```powershell
# Check update frequency (in config.yaml)
check_updates: true
update_channel: "stable"  # or "beta"
```

---

## Time Machine & History (Phase 2)

### `odin history`

View the history of environment snapshots with a colored timeline.

**Features:**
- **Timeline view**: Chronological list of all snapshots
- **Change tracking**: See what changed between snapshots
- **Detailed diffs**: Environment variables, packages, extensions, Git config
- **Time indicators**: Relative timestamps (e.g., "2 days ago")
- **JSON export**: Export history data for analysis

**What it tracks:**
- Added/removed packages
- Changed environment variables
- Added/removed VS Code extensions
- Git configuration changes

**Usage:**
```powershell
# View snapshot history
odin history

# Show detailed changes between snapshots
odin history --detailed

# Export as JSON
odin history --json
```

**Example output:**
```
📜 Snapshot History (newest first)
════════════════════════════════════════════════════════════════

📦 Snapshot: snapshot-20250313-120000
   ↳ 2 hours ago on WORKSTATION-01
   Changes:
   ✓ 3 packages added (node 20.11, eslint 8.55, prettier 3.2)
   ~ 1 package updated (git 2.44 → 2.45)
   ✗ 2 packages removed (legacy-tool, deprecated-pkg)
   ↔ 4 environment variables changed
   ⚙ 2 VS Code extensions installed

📦 Snapshot: snapshot-20250312-090000
   ↳ 1 day ago on WORKSTATION-01
   Changes:
   ✓ 5 packages added (python 3.12, nodejs 20, rust 1.75)
   ↔ 3 environment variables changed

📦 Snapshot: snapshot-20250311-150000
   ↳ 2 days ago on WORKSTATION-01
   (initial snapshot)
```

---

### `odin rollback`

Restore environment to a previous snapshot.

**Features:**
- **Dry-run by default**: Preview changes before applying
- **Full restoration**: Packages, environment variables, extensions, Git config
- **Safety first**: Requires `--apply` flag to confirm
- **Selective rollback**: Can rollback to any snapshot in history
- **Change preview**: Shows what will be restored

**Important:**
- Does NOT uninstall packages added after the snapshot
- Only restores configuration and missing packages
- Git config restored only for global settings

**Usage:**
```powershell
# Preview rollback (dry-run)
odin rollback snapshot-20250312-090000

# Apply rollback with confirmation
odin rollback snapshot-20250312-090000 --apply

# Show as JSON
odin rollback snapshot-20250312-090000 --json
```

**Workflow example:**
```powershell
# 1. Check history
odin history

# 2. Preview rollback (no changes made)
odin rollback snapshot-20250312-090000
# Output:
# 🔙 Rollback Preview
# Rolling back to: snapshot-20250312-090000 (2 days ago)
# 
# Changes to Apply:
#   → 5 packages to install
#   → 3 environment variables to restore
#   → 2 VS Code extensions to install
#   → Git config entries to restore
# 
# Preview mode - no changes applied. Use --apply to rollback.

# 3. Apply rollback
odin rollback snapshot-20250312-090000 --apply
```

**Use cases:**
- Quick recovery from environment issues
- Testing different tool configurations
- Comparing machine state over time
- Auditing what changed between versions

---

## Configuration

### `odin config`

Manage Odin configuration and settings.

**Subcommands:**
- `odin config show` - Display current configuration
- `odin config github` - Setup GitHub integration

**Location:**
```
~/.odin/config.yaml
```

**Example configuration:**
```yaml
github:
  owner: "your-username"
  repo: "odin-state"
  branch: "main"
  token: (stored in credential manager)

snapshots:
  auto_backup: true
  backup_frequency: 24h  # hours between auto-sync

update:
  check_updates: true
  update_channel: "stable"
  auto_update: false
```

**Usage:**
```powershell
# Show current config
odin config show

# Show as JSON
odin config show --json

# Interactive GitHub setup
odin config github

# Setup and sync immediately
odin config github --sync-now

# Clear GitHub configuration
odin config github --clear
```

---

### `odin init`

Initialize Odin configuration.

**Creates:**
- `~/.odin/` directory
- `~/.odin/config.yaml` with default settings
- Initial snapshot files

**Usage:**
```powershell
# Initialize Odin
odin init

# Init with custom Odin directory
$env:ODIN_DIR = "D:\backups\odin"
odin init
```

---

## Dashboard

### Interactive Dashboard Features

**Main view:**
- System information (OS, CPU, RAM)
- Latest snapshot metadata
- Package counts by manager
- Recent changes summary

**Sections:**

**System Health**
```
OS:       Windows 11 23H2
Hostname: WORKSTATION-01
CPU:      12 cores @ 3.6 GHz
RAM:      32 GB
Disk:     512 GB (73% used)
```

**Snapshots**
```
Last Snapshot:  2 hours ago
Packages:       127 installed (winget: 45, choco: 32, scoop: 50)
VS Code Ext:    42 extensions
Git Config:     Configured
```

**Recent Changes**
```
[NEW] Microsoft.PowerShell v7.4.0
[REMOVED] OldPackage v1.0.0
[UPDATED] Python 3.10 → 3.11.0
```

**Sync Status**
```
Repository:     https://github.com/user/odin-state
Last Sync:      15 minutes ago
Branch:         main
Status:         ✓ In sync
```

---

## Command Reference

### All Commands Summary

```
ENVIRONMENT & SNAPSHOT:
  snapshot          Capture machine state
  restore           Restore from snapshot (preview mode by default)
  diff              Compare live state vs snapshot
  export            Generate restore scripts
  init              Initialize configuration

PROCESS & PORT MANAGEMENT:
  ports             List listening ports
  ps                Interactive process dashboard
  kill              Kill process by port/PID
  
DIAGNOSTICS:
  doctor            Health check
  dashboard         Status dashboard
  config            Manage configuration
  
SYNC & UPDATES:
  sync / backup     Push snapshot to GitHub
  update            Check and install updates
```

### Global Options

```
--odin-dir <DIR>    Override ~/.odin directory (env: ODIN_DIR)
--json              Output as JSON
--help              Show help
--version           Show version
```

---

## Feature Ideas for Enhancement

This documentation provides all existing features. Here are some potential future enhancements:

### Monitoring & Alerts
- **Auto-sync on changes**: Automatically backup when machine state changes
- **Change notifications**: Email/Slack alerts for significant changes
- **Threshold alerts**: Alert when disk/memory reaches limits
- **Service health**: Monitor critical services (databases, web servers)

### Advanced Process Management
- **Process grouping**: Group related processes (VS Code extensions, node modules)
- **Resource history**: Track CPU/memory usage over time
- **Process dependencies**: Show which processes depend on each other
- **Custom process rules**: Kill/restart processes based on patterns

### System Optimization
- **Cleanup tools**: Remove temporary files, caches
- **Startup optimization**: Disable unnecessary startup programs
- **Memory tuning**: Suggest memory optimization based on usage
- **Disk analysis**: Find large unused directories

### Environment Cloning
- **Team environments**: Share snapshots with team members
- **Remote restore**: Clone environment to remote machines
- **Docker integration**: Export environment as Docker container
- **VM templates**: Create VM images from snapshots

### Advanced Diagnostics
- **Performance profiling**: Detailed performance analysis
- **Dependency tracking**: Map package dependencies
- **Version compatibility**: Check incompatible package versions
- **Registry scanning**: Diagnose registry issues

### Integration Features
- **IDE integration**: VS Code extension for Odin
- **CI/CD integration**: GitHub Actions for auto-backup
- **Webhook support**: Trigger syncs on external events
- **Plugin system**: Custom commands and integrations

---

## Architecture Notes

Odin's architecture is designed for extensibility:

- **Services layer**: Business logic separated from commands
- **Integrations layer**: Platform-specific code (Windows, Git, GitHub)
- **Models**: Strongly typed serde-compatible data structures
- **UI layer**: Ratatui-based interactive dashboards

Future features can be added by:
1. Creating new services in `src/services/`
2. Adding platform code to `src/integrations/`
3. Creating new command handlers in `src/commands/`
4. Adding UI components to `src/ui/`

---

**Last Updated**: May 2024  
**Version**: 0.1.0  
**Repository**: https://github.com/AsimAftab/Project-Odin
