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
# Preview what would be restored (from the current vault)
odin restore

# Apply all changes (install packages, restore config)
odin restore --apply

# Restore a specific local history snapshot
odin restore <snapshot-id> --apply

# Restore a snapshot hosted on the Odin Platform — no local history needed.
# If <snapshot-id> isn't found locally, Odin fetches it from the platform
# (requires `odin login`) and restores it the same way.
odin restore <platform-snapshot-id> --apply
```

This is the native alternative to downloading a "Restore script" from the
dashboard: it goes through the same package-manager gating, skip-if-installed
logic, and `RestoreConfig` scoping as a local restore — the exported `.ps1`
script doesn't check what's already installed.

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

### Backup to Odin Platform

Connect this machine to your Odin Platform account with a browser-based login
(OAuth 2.0 device flow — no token copy/paste):

```powershell
# Opens your browser to approve this machine, then stores the token securely
odin login --url https://your-platform.example.com
```

Odin verifies the connection, then asks whether to (1) upload each new snapshot
automatically and (2) upload your existing local snapshots now. The API token
(format `odin_<keyId>_<secret>`) is kept in the Windows credential store — never
in `config.yaml`. Secret-looking environment values are redacted before upload:
both by variable name (containing `TOKEN`, `KEY`, `SECRET`, `PASSWORD`, …) and by
value shape (GitHub tokens, `sk-…` keys, AWS keys, JWTs, PEM private keys), so a
secret stored under a benign name is still masked.

```powershell
# Upload snapshots on demand
odin push            # upload the latest snapshot
odin push --all      # upload every snapshot in local history

# Capture and upload in one step (even if auto-upload is off)
odin snapshot --push

# Skip the upload for a single snapshot when auto-upload is on
odin snapshot --no-push

# Always-on sync: with auto-upload enabled, drift is snapshotted and uploaded
odin watch --follow

# Check the connection / who you're connected as
odin config show

# Disconnect this machine (local snapshots are untouched)
odin logout
```

For CI or headless machines, connect with a pre-minted token instead of the
browser flow:

```powershell
odin config platform --url https://your-platform.example.com --token odin_xxxx... --non-interactive
```

Uploads never modify or delete local snapshots — a failed upload leaves your
vault intact and prints an `odin push` retry hint.

### Scheduled Snapshots

Register a recurring snapshot with the Windows Task Scheduler (per-user task, no
admin required):

```powershell
# Daily at 09:00, uploading each snapshot to the platform
odin schedule enable --interval daily --time 09:00 --push

# Every hour, local snapshot only
odin schedule enable --interval hourly

# Is a scheduled task registered?
odin schedule status

# Remove it
odin schedule disable
```

The task runs `odin snapshot [--push]` as the current user. This survives
reboots, unlike the foreground `odin watch --follow` drift monitor.

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

The durable way is a scheduled task (survives reboots):

```powershell
odin schedule enable --interval daily --push
```

Alternatively, add to your PowerShell `$PROFILE` to back up on each login:

```powershell
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

