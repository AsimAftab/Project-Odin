# Changelog

All notable changes to the Odin CLI are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [SemVer](https://semver.org/) (pre-1.0: minor bumps may
break). Releases are cut by pushing a `v*.*.*` tag (see `docs/release.md`).

## [Unreleased]

## [0.12.0] — 2026-07-11

### Changed

- PATH restore now merges into the live user PATH (with a pre-restore backup
  in `~/.odin/logs/`) instead of replacing it.
- Security: updated `crossbeam-epoch` and `quinn-proto` for RUSTSEC-2026-0204
  and RUSTSEC-2026-0185.

### Added

- Restore sections `terminal` and `ps-profile`: Windows Terminal settings and
  the PowerShell profile are now restored, not just captured (with backups of
  the existing files).
- OSS hygiene: code of conduct, issue/PR templates, CODEOWNERS, dependabot,
  MSRV (1.88) + `cargo audit` enforced in CI.

## [0.11.0] — 2026-07-05

- Restore continue-on-error by default (`--fail-fast` opts out), restore-report
  polish, platform snapshot pull via `odin restore <id>`.

## [0.10.x] — 2026-07-05

- Winget msstore `--source` ambiguity fix, restore-script guards, keyed platform
  tokens.

## [0.9.0] / [0.8.0] — 2026-07-04

- Unified `sync`, smoother `login` (RFC 8628 device flow), profile upload,
  PID-tracked Asgard layouts, platform connection (`odin login` + snapshot sync).

## [0.6.x] — 2026-06

- Asgard layout activation improvements, snapshot retention (`keep_last`),
  env-restore fix, npm/pip/cargo package tracking.

## [0.5.x] — 2026-05

- Multi-monitor Asgard layouts.

## [0.4.0] — 2026-05-18

- Odin mythology theming; `kill` → `freeport`, `dashboard` → `all-eye`.

Earlier history: see `git log`.
