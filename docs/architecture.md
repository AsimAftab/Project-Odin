# Odin Architecture

Odin is organized around commands, services, strongly typed models, platform integrations, and terminal UI.

```text
src/
  commands/       CLI command handlers (snapshot, restore, sync, ports, ps, kill, etc.)
  core/           application context and typed errors
  integrations/   Windows tools, Git, GitHub, PowerShell, VS Code, Process management
  models/         serde-compatible snapshot and config types
  services/       snapshot, restore, sync, config, secrets, storage, process management
  ui/             Ratatui dashboard views (main dashboard, process dashboard)
  utils/          filesystem, logging, checksums, banner, terminal helpers
```

## Command Layer (`src/commands/`)

Thin CLI handlers that parse arguments and delegate to services.

**Environment & Snapshot:**
- `snapshot.rs` - Capture machine state
- `restore.rs` - Restore from snapshot
- `diff.rs` - Compare live vs snapshot
- `export.rs` - Generate restore scripts
- `init.rs` - Initialize config

**Process & Port Management:**
- `ports.rs` - List listening ports with process info
- `ps.rs` - Launch interactive process dashboard
- `kill.rs` - Kill processes by port/PID

**Diagnostics & Sync:**
- `dashboard.rs` - Show status dashboard
- `doctor.rs` - Health check
- `sync.rs` - GitHub backup
- `config.rs` - Configuration management
- `update.rs` - Check and install updates

## Service Layer (`src/services/`)

Core business logic and workflows.

**State Management:**
- `snapshot_service.rs` - Collect machine state, generate restore scripts
- `restore_service.rs` - Read snapshots and stage package installs
- `storage.rs` (SnapshotStore) - Canonical snapshot file paths and persistence
- `sync_service.rs` - Git repo management and GitHub integration

**Configuration & Secrets:**
- `config_service.rs` - Manage `config.yaml`
- `secret_service.rs` - Store GitHub token in OS credential store
- `update_service.rs` - Check GitHub Releases, download updates

**Diagnostics:**
- `doctor_service.rs` - Health checks and diagnostics
- `diff_service.rs` - State comparison

**NEW - Process Management:**
- `process_service.rs` - Port/process discovery and management
  - `get_listening_ports()` - List all listening ports via netstat
  - `find_process_by_pid()` - Lookup process info
  - `kill_process()` - Safe process termination
  - `get_all_processes()` - Snapshot all running processes with resource metrics

## Integration Layer (`src/integrations/`)

Platform-specific code for Windows, Git, GitHub, and system tools.

**Windows System Integrations:**
- `process.rs` - NEW: netstat parsing, taskkill execution, sysinfo caching
  - `get_listening_ports()` - Parse netstat output, resolve process names (50-100x faster with sysinfo caching)
  - `find_process_by_port()` - Find PID from port
  - `kill_process_by_id()` - Execute taskkill /PID
  - `ProcessInfo`, `PortInfo` data models
- `package_managers.rs` - winget, Chocolatey, Scoop discovery
- `vscode.rs` - VS Code and extension discovery
- `powershell.rs` - PowerShell profile discovery
- `windows_terminal.rs` - Windows Terminal settings
- `process.rs` - General process execution with exit code handling

**Git & GitHub:**
- `git.rs` - Git config discovery and restoration
- `github.rs` - GitHub API for releases and sync
- `sync.rs` - Push snapshots as git commits

## Models Layer (`src/models/`)

Strongly-typed serde models for snapshot and config data.

**Snapshot Models:**
- `machine.rs` - OS, CPU, memory, disk info
- `environment.rs` - Environment variables, PATH
- `packages.rs` - Package manager data
- `git.rs` - Git configuration
- `vscode.rs` - VS Code extensions

**NEW - Process Models:**
- `process.rs`
  - `ProcessInfo` - PID, name, status, resource usage
  - `PortInfo` - Port, protocol, address, associated process
  - `ProcessStats` - CPU %, memory, threads, status

**Configuration Models:**
- `config.rs` - User configuration structure
- `report.rs` - Doctor/diff report output

## UI Layer (`src/ui/`)

Ratatui-based interactive terminal dashboards.

**Main Dashboard:**
- `dashboard.rs` - Status overview with snapshot metadata

**NEW - Process Dashboard:**
- `process_dashboard.rs` - Interactive process monitor (htop-style)
  - Real-time process list with sorting/filtering
  - Keyboard controls (arrow keys, K to kill, 1-4 to sort)
  - Resource metrics (CPU %, memory, threads)
  - Safe process killing with confirmation
  - 500ms refresh rate for live updates

## Utilities (`src/utils/`)

Helper functions and terminal utilities.

- `filesystem.rs` - Directory/file operations
- `logging.rs` - Structured logging
- `checksum.rs` - File integrity verification
- **NEW - `banner.rs`** - Colorful ASCII art banner with command list
- `terminal.rs` - Terminal detection and helpers

## Data Flow

### Process Management Flow

```
Command Layer
  ports.rs ─→ ProcessService ─→ Integration Layer
  ps.rs    ─→ ProcessService ─→ Integration Layer  
  kill.rs  ─→ ProcessService ─→ Integration Layer

ProcessService (business logic)
  ├─ get_listening_ports() ──→ process::get_listening_ports()
  ├─ find_process_by_pid()  ──→ sysinfo::System
  ├─ kill_process()         ──→ process::kill_process_by_id()
  └─ get_all_processes()    ──→ sysinfo::System

Integration Layer (Windows system calls)
  ├─ netstat -ano          (parse port output)
  ├─ taskkill /PID /F      (kill process)
  ├─ sysinfo::System       (efficient process metrics cache)
  └─ WMI via PowerShell    (process name resolution)

UI Layer (if interactive)
  └─ process_dashboard.rs ──→ ratatui rendering ──→ terminal
```

### Snapshot & Restore Flow

```
Command Layer
  snapshot.rs ──→ SnapshotService ──→ Integrations ──→ Storage

SnapshotService (orchestration)
  ├─ collect_machine_info()
  ├─ collect_packages()
  ├─ collect_vscode()
  ├─ collect_git()
  └─ generate_restore_scripts()

Integrations (discovery)
  ├─ package_managers (winget, choco, scoop)
  ├─ vscode (VS Code extensions)
  ├─ git (config files)
  └─ windows_terminal (settings)

Storage (SnapshotStore)
  ├─ machine.json
  ├─ packages.json
  ├─ vscode_extensions.json
  ├─ git_config.json
  └─ restore.ps1
```

## Key Design Decisions

### Windows-First with Extensibility
- Integration layer isolated under `integrations/` for future Linux/macOS support
- Platform-specific code doesn't leak into services or models
- Snapshot models are platform-agnostic

### Safety Defaults
- `odin restore` is dry-run by default (`--apply` required)
- `odin kill` requires `--force` flag
- Interactive mode is explicit (TUI dashboards)
- No destructive operations without confirmation

### Efficient Process Discovery
- Uses `sysinfo` crate for cached process metrics (50-100x faster than WMI queries per-process)
- netstat stdout parsing for port discovery
- Smart caching layer to avoid redundant system calls

### Strongly Typed Data
- All snapshot data is serde-compatible JSON
- Config is YAML (human-editable, type-safe struct)
- Reports are JSON-compatible for scripting
- No stringly-typed data in core logic

### Modularity & Extensibility
- Services expose clean, single-responsibility functions
- Integrations are pluggable (new tool = new integration module)
- UI components are isolated from business logic
- Commands are thin dispatchers (no business logic in handlers)

## Interaction Patterns

### Add a New Command

1. Create `src/commands/my_command.rs` with handler function
2. Add command enum variant to `src/cli.rs`
3. Add args struct if needed
4. Call `MyService::do_something()` from command handler
5. Return result (text or JSON)

### Add a New Service

1. Create `src/services/my_service.rs`
2. Implement struct with public functions
3. Call integrations as needed
4. Return strongly-typed models
5. Export from `src/services/mod.rs`

### Add a New Integration

1. Create `src/integrations/my_tool.rs`
2. Implement tool discovery/management
3. Return integration-specific models or errors
4. Call from services, not commands
5. Export from `src/integrations/mod.rs`

## Technology Stack

- **Language**: Rust 2021 edition
- **CLI**: Clap for argument parsing
- **TUI**: Ratatui for interactive dashboards
- **System**: sysinfo for process/system metrics
- **Serialization**: serde with JSON/YAML
- **Colors**: colored crate for terminal colors
- **Async**: tokio for async workflows
- **Process**: subprocess via std::process with PowerShell fallback

---

Windows integration is centralized and isolated, making Linux and macOS support straightforward when needed.

