# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, lint, and test

CI (`.github/workflows/ci.yml`) runs on `windows-latest`. Match it locally:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
```

Run a single test by name filter: `cargo test <name_substring>`.

Run the CLI during development: `cargo run -- <subcommand>` (e.g. `cargo run -- snapshot`, `cargo run -- update --check`).

Clippy is gated with `-D warnings`, so any new warning fails CI.

## What Odin is

Windows-first Rust CLI that snapshots developer-workstation state (machine info, env/PATH, package managers, VS Code extensions, Git config, PowerShell/Windows Terminal settings) into `%USERPROFILE%\.odin` as plain JSON, then restores, diffs, syncs to a private GitHub repo, and reports diagnostics. Also includes process/port management (`ports`, `ps`, `kill`) and time-machine history (`history`, `rollback`).

The `.odin` directory location can be overridden globally via `--odin-dir` or `ODIN_DIR`.

## Architecture: commands → services → integrations

`src/main.rs` parses CLI with Clap, builds `core::context::AppContext` (which loads `~/.odin/config.yaml` if present), and dispatches to `commands::<name>::run(ctx, args)`.

The four-layer split is load-bearing — keep new code in the right layer:

- **`src/commands/*`** — thin handlers. Parse args/output mode (`--json`, `--apply`, etc.) and delegate. Do not put business logic here.
- **`src/services/*`** — workflows and orchestration. This is where behavior changes belong.
- **`src/integrations/*`** — anything that touches the OS, an external tool, or a network API (winget/choco/scoop, VS Code, PowerShell, Windows Terminal, git, GitHub API, netstat/taskkill, sysinfo).
- **`src/models/*`** — serde-compatible types shared across layers.
- **`src/ui/dashboard.rs`** and **`src/ui/process_dashboard.rs`** — Ratatui TUIs; commands fall back to plain text when stdout is non-interactive.

Snapshot persistence is centralized in `src/services/storage.rs::SnapshotStore`. The canonical filenames live there as `pub const` (`MACHINE`, `ENV`, `PACKAGES`, `VSCODE`, `GIT`, `LOCK`) — use them, do not hardcode `"machine.json"` etc. elsewhere. `odin.lock` is a sha256 manifest of the snapshot files, regenerated on every write.

External command execution must go through `integrations::process::{capture, checked}` (centralizes exit-code handling and Windows `.cmd`/`.bat` shim — direct `std::process::Command` calls bypass that and break on Windows script targets).

## Conventions that bite if ignored

- **Dry-run defaults are a contract.** `odin restore` must remain dry-run unless `--apply`. `odin freeport` (alias `kill`) requires `--force`. `odin update --check` is non-mutating; `odin update` stages a binary swap that finalizes only after process exit. Don't change these defaults without explicit instruction.
- **Output contracts for automation.** `doctor --json`, `diff --json`, `config show --json`, `ports --json` emit structured JSON; the default mode emits human-readable text with `colored` status labels. Keep both paths working when modifying these commands.
- **Case-insensitive identity comparison.** When deduplicating or matching package IDs, env-var names, or VS Code extension IDs, use `to_ascii_lowercase` / `eq_ignore_ascii_case` — that's how existing diff/restore code aligns live state to snapshots.
- **Discovery is failure-tolerant.** Integrations under `package_managers`, `vscode`, `git_cli` degrade missing tools to empty results with warnings rather than aborting the snapshot. Follow this pattern; one missing tool should not poison the whole `odin snapshot` run.
- **Secrets in OS credential store, not config.** GitHub tokens go through `services::secret_service` (uses `keyring` with `windows-native`). `config.yaml` holds non-secret config only.
- **Interactive vs non-interactive flows.** `odin all-eye` (alias `dashboard`) and `odin config github` have both; `--non-interactive` and `dialoguer` paths must both work. Detect TTY via `utils::terminal`.

## Adding things

- **New command:** add a variant to `Commands` in `src/cli.rs`, an args struct, a `commands/<name>.rs` with `pub async fn run(ctx: AppContext, args: ...) -> Result<()>`, and wire it in `main.rs`. Keep it thin; put logic in a service.
- **New service:** create `services/<name>_service.rs`, export from `services/mod.rs`, return typed models from `models/`.
- **New integration:** create `integrations/<tool>.rs`, export from `integrations/mod.rs`, call only from services (never from commands).

## Release surface

`scripts/install.ps1`, `scripts/uninstall.ps1`, and `scripts/bootstrap.ps1` are the user-facing install lifecycle (PATH setup, in-place upgrade staging, uninstall). `.github/workflows/release.yml` builds `odin.exe`, packages `odin-windows-x64.zip` with the install scripts and `checksums.txt`, and publishes to GitHub Releases. `update_service` reads from that same release feed — versioning and asset naming there are coupled.
