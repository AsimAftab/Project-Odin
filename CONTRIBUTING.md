# Contributing to Odin

Thanks for your interest in improving Odin! This repo (`Project-Odin`) is the
Windows-first workstation snapshot/restore/diagnostics CLI. Its companion is the
[Odin Platform](https://github.com/AsimAftab/Odin-Platform) backend + dashboard.
Contributions of all kinds are welcome: bug reports, docs, features, and fixes.

## Local setup

Requirements: a recent stable [Rust toolchain](https://rustup.rs) (2021 edition).
Odin is Windows-first — it uses the `windows` crate and shells out to PowerShell,
winget, `schtasks`, etc. Build and run on Windows for full functionality.

```powershell
cargo build
cargo run -- snapshot        # run a subcommand
```

## Before opening a PR

The CI gate (`.github/workflows/ci.yml`) runs on Windows and must pass:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

- Keep changes focused; match the surrounding style (Norse-mythology naming is
  intentional — see the glossary in `README.md`).
- Add unit tests for new parsing/logic. Prefer pure functions that take the raw
  input (see the package-manager parsers and `services/redact.rs`) so they test
  without touching the system.
- Never commit secrets. API/GitHub tokens live in the OS credential store, never
  in `config.yaml`.

## Changes that touch the platform

The CLI talks to [Odin Platform](https://github.com/AsimAftab/Odin-Platform) over
a small contract: the ingest payload (`POST /api/ingest`), the device-auth flow
(`/api/device/*`), and the opaque API token. If you change any of those, open the
matching PR in the platform repo and update the mirrored docs
(`docs/odin-platform-spec.md` / `odin-platform-tasks.md` — edit them in both repos
together).

## Adding a package manager

`src/integrations/package_managers.rs` follows a consistent shape: a `list_X`
function that shells out, delegating parsing to a pure `parse_X` you can unit
test with a fixture string. Wire the new manager into `list_packages`,
`detect_managers`, the `PackageManager` enum (`src/models/package.rs`),
`source_enabled` (`src/services/restore_service.rs`), and the default
`package_managers` list (`src/models/config.rs`).

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](./LICENSE).
