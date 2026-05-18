# Usage Guide

Quick reference for common Odin workflows.

## Getting Started

Initialize Odin:

```powershell
odin init
```

This creates `~/.odin/config.yaml` and initializes the snapshot directory.

## Core Workflows

### Capture & Monitor Your Environment

```powershell
# Take a snapshot of your machine
odin snapshot

# View the snapshot status (Hliðskjálf — interactive overview)
odin all-eye         # alias: odin dashboard

# Check machine health
odin doctor
```

### Process & Port Management

```powershell
# List all listening ports
odin ports

# See which process is using port 3000
odin ports | grep 3000

# Open interactive process dashboard (like htop)
odin ps

# Free a port (requires --force) — alias: kill
odin freeport 3000 --force

# Free a port by PID
odin freeport 1234 --force

# Preview what would be freed (dry-run, no --force needed)
odin freeport 8080
```

### Restore Your Environment

```powershell
# Preview what would be restored
odin restore

# Apply all changes (install packages, restore config)
odin restore --apply
```

### Compare Changes

```powershell
# See what changed since last snapshot
odin diff

# View differences as JSON
odin diff --json
```

### Backup to GitHub

First, configure GitHub:

```powershell
# Interactive setup (stores token in credential manager)
odin config github

# Or configure and sync in one step
odin config github --sync-now
```

Then sync your snapshots:

```powershell
# Push snapshots to GitHub
odin sync

# Alias for sync
odin backup

# Sync to specific branch
odin sync --branch staging

# Sync to custom remote
odin sync --remote https://github.com/you/private-odin-state.git
```

### Check for Updates

```powershell
# Check if update available
odin update --check

# Install latest version
odin update
```

## Advanced Usage

### Export Restore Scripts

```powershell
# Generate bootstrap and restore scripts
odin export
```

This creates:
- `restore.ps1` - Installs packages and restores config
- `install.ps1` - Sets up Odin executable
- `bootstrap.ps1` - Downloads and installs Odin

### Configuration Management

```powershell
# Show current configuration
odin config show

# Show configuration as JSON
odin config show --json
```

### Automated Backup

Set up GitHub integration and then add to PowerShell profile:

```powershell
# Add to $PROFILE
# Auto-backup on login
odin snapshot
odin sync
```

### Monitor System Health

```powershell
# Interactive process monitoring (live updates)
odin ps

# Diagnose issues
odin doctor

# Check for resource-heavy processes
odin ports
```

## Tips & Tricks

### Find Process Using Port

```powershell
# See all listening ports
odin ports

# Find specific port
odin ports | grep ":8080"
```

### Kill Stuck Processes

```powershell
# Interactive: use 'odin ps', press K to kill
odin ps

# Or direct command
odin freeport 3000 --force
```

### Backup Everything

```powershell
# Snapshot current state
odin snapshot

# Backup to GitHub
odin sync
```

### Restore on New Machine

```powershell
# Clone your odin-state repo
git clone https://github.com/you/odin-state.git ~/.odin

# Restore everything
odin restore --apply
```

### Monitor Development Servers

```powershell
# Open process dashboard with live metrics
odin ps

# Or check specific port
odin ports | grep "8080"
```

## Command Examples

```powershell
# Daily workflow
odin snapshot
odin all-eye                  # alias: odin dashboard
odin doctor

# Port management
odin ports                    # List all ports
odin freeport 5432 --force    # Free postgres on port 5432
odin ps                       # Interactive process view

# GitHub backup
odin config github           # First time setup
odin sync                    # Regular backups

# Troubleshooting
odin doctor                  # Diagnose issues
odin diff                    # See what changed
odin restore                 # Preview restore

# Maintenance
odin update --check          # Check for updates
odin update                  # Install update
```

## JSON Output for Automation

All commands support `--json` for scripting:

```powershell
# JSON snapshot
$snapshot = odin snapshot --json | ConvertFrom-Json

# JSON ports (for parsing)
$ports = odin ports --json | ConvertFrom-Json

# JSON health check
$health = odin doctor --json | ConvertFrom-Json

# JSON processes
$processes = odin ps --json | ConvertFrom-Json
```

## See Also

- [Features Guide](features.md) - Complete feature documentation
- [Architecture](architecture.md) - How Odin works
- [GitHub Sync Setup](github.md) - Private repository backup
- [Release Process](release.md) - Building and publishing

