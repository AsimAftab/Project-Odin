# Copilot Instructions for Project Odin

## Build, test, and lint commands

Use the same commands as CI (`.github/workflows/ci.yml`) on Windows:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
```

Run one test by name filter:

```powershell
cargo test <test_name>
```

Run the CLI locally during development:

```powershell
cargo run -- <subcommand>
```

Example:

```powershell
cargo run -- snapshot
```

## High-level architecture

Odin is a Windows-first Rust CLI that snapshots and restores developer workstation state into `%USERPROFILE%\.odin` (or `--odin-dir` / `ODIN_DIR` override).

- `src/main.rs` is the entrypoint: parse CLI with Clap, build `AppContext`, dispatch to command handlers.
- `src/commands/*` are thin handlers. They parse command args/output mode and delegate business logic to services.
- `src/services/*` contains core workflows:
  - `snapshot_service`: collects machine/env/packages/VS Code/git state via integrations, writes snapshot JSONs, then generates restore/export scripts.
  - `restore_service`: reads snapshot files and prints a dry run by default; `--apply` executes package installs, git config restoration, VS Code extension installs, and environment writes.
  - `sync_service`: manages `%USERPROFILE%\.odin` as a git repo, commits snapshot changes, and pushes to configured remote/branch.
  - `update_service`: checks GitHub Releases, compares versions, downloads latest `odin.exe`, and stages safe replacement after process exit.
  - `doctor_service` and `diff_service`: compare live machine state vs snapshot and produce human-readable or JSON output.
  - `config_service`/`secret_service`: persist config in `config.yaml`; store GitHub token in OS credential store.
- `src/services/storage.rs` (`SnapshotStore`) is the canonical snapshot persistence layer and file naming source:
  - `machine.json`, `env.json`, `packages.json`, `vscode_extensions.json`, `git_config.json`, `odin.lock`
- `src/integrations/*` wraps system dependencies (git, GitHub API, package managers, PowerShell, VS Code, Windows environment).
- `scripts/install.ps1`, `scripts/uninstall.ps1`, `scripts/bootstrap.ps1` define the Windows global install lifecycle (PATH setup, upgrades, uninstall, bootstrap entrypoint).
- `src/models/*` defines serde snapshot/config/report types shared across services and integrations.
- `src/ui/dashboard.rs` provides interactive TUI rendering when a terminal is interactive; command falls back to plain-text output otherwise.

## Key repository conventions

- Keep command modules thin and put behavior changes in services/integrations rather than inside `src/commands/*`.
- Use `SnapshotStore` constants and helpers for snapshot file paths instead of hardcoding filenames in multiple places.
- Route external command execution through `integrations::process::{capture, checked}`; this centralizes exit-code handling and Windows `.cmd/.bat` support.
- Preserve safety defaults:
  - `odin restore` is dry-run unless `--apply` is set.
  - `odin dashboard`/`odin config github` have interactive and non-interactive flows; keep both paths working.
  - `odin update --check` must remain non-mutating; `odin update` stages binary replacement and requires process exit to finalize.
- For machine-state comparison/deduplication, follow existing normalization: compare package IDs, env names, and extension IDs case-insensitively (`to_ascii_lowercase` / `eq_ignore_ascii_case`).
- Follow existing error-tolerance patterns in discovery integrations (`package_managers`, `vscode`, `git_cli`): probe failures generally degrade to empty snapshots with warnings rather than aborting the whole snapshot workflow.
- Keep output contracts stable for automation:
  - `doctor --json`, `diff --json`, `config show --json` emit structured JSON.
  - default mode emits human-readable text with `colored` status labels.
