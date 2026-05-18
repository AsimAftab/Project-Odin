# Project Odin - Windows Developer Environment Manager

Odin is a specialized CLI tool designed for Windows developers to snapshot, restore, sync, and diagnose their workstation environments. It captures machine state, package manager configurations (winget, chocolatey, scoop), shell profiles, Git settings, VS Code extensions, and more into portable JSON snapshots.

## 🚀 Quick Start

- **Initialize:** `odin init`
- **Capture State:** `odin snapshot`
- **Sync to GitHub:** `odin sync --remote <url>`
- **Restore:** `odin restore --apply`
- **Interactive Dashboard:** `odin all-eye` (alias: `odin dashboard`)
- **Process Monitoring:** `odin ps`

## 🛠 Building and Running

### Prerequisites
- **Rust:** Latest stable version (Edition 2021)
- **Windows:** Primary target OS

### Key Commands
- **Build:** `cargo build` (Debug) or `cargo build --release` (Optimized)
- **Test:** `cargo test`
- **Run Locally:** `cargo run -- <command>`
- **Format Code:** `cargo fmt`
- **Lint:** `cargo clippy`

### Installation Scripts
- `.\scripts\install.ps1`: Installs the binary locally or system-wide.
- `.\scripts\bootstrap.ps1`: Downloads and installs Odin from GitHub.
- `.\scripts\uninstall.ps1`: Removes Odin from the system.

## 🏗 Architecture & Design

The project follows a modular service-oriented architecture:

- **Command Layer (`src/commands/`):** CLI entry points using `clap`. They delegate all logic to services.
- **Service Layer (`src/services/`):** Core business logic (snapshotting, restoring, syncing, etc.).
- **Integration Layer (`src/integrations/`):** Platform-specific code for interacting with Windows, package managers, Git, and GitHub.
- **Models Layer (`src/models/`):** Strongly-typed `serde` models for configuration and snapshot data.
- **UI Layer (`src/ui/`):** Interactive TUI components built with `ratatui`.
- **Core (`src/core/`):** Application context (`AppContext`) and error handling.
- **Utils (`src/utils/`):** Shared helpers for logging, filesystem, and terminal.

## 📋 Development Conventions

### Coding Style
- **Rust Idioms:** Use standard Rust patterns. Error handling via `anyhow` for top-level and `thiserror` for library-style errors.
- **Safety First:** Destructive operations (like `restore` or `kill`) must have dry-run defaults or require explicit flags (e.g., `--apply`, `--force`).
- **Platform Isolation:** Keep Windows-specific logic within the `integrations` layer to allow for future cross-platform support.

### Testing
- **Unit Tests:** Located within source files or sibling modules.
- **Integration Tests:** Use `tempfile` for testing filesystem-dependent logic.
- **Verification:** Always run `cargo test` before submitting changes.

### Adding New Features
1. **Model:** Define data structures in `src/models/`.
2. **Integration:** Add low-level tool interaction in `src/integrations/`.
3. **Service:** Implement business logic in `src/services/`.
4. **Command:** Expose the feature via a new command in `src/commands/` and update `src/cli.rs`.

## 📂 Project Structure

- `docs/`: Comprehensive documentation (Architecture, Features, Usage).
- `examples/`: Sample configuration files.
- `scripts/`: PowerShell scripts for installation and bootstrapping.
- `target/`: Build artifacts (ignored by git).

## 🛡 Security
- **Secrets:** GitHub tokens are stored in the Windows Credential Store using the `keyring` crate. NEVER log or print secrets.
- **Privacy:** Snapshots are stored locally in `~/.odin` by default and should only be synced to private repositories if they contain sensitive path information.
