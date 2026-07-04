<!-- Mirrored in both repos (Odin-Platform and Project-Odin). Edit together. -->

# Odin Platform Product Spec

## Summary

Odin Platform is the account-based hub for backing up, reviewing, restoring, and migrating developer workstation state captured by the open-source Odin CLI. The CLI remains the local agent and portable source of truth; the platform makes that state easier to manage across machines without forcing users into GitHub as the only remote.

The core experience:

1. A user creates an Odin Platform account (self-hosted Better Auth email/password).
2. The user installs Odin with `winget install AsimAftab.Odin`.
3. The user pairs the CLI via `odin login` (browser device flow) or an API token.
4. `odin snapshot --push` captures local workstation state and uploads it to the user's account.
5. The dashboard shows machines, snapshots, packages, VS Code extensions, Git config, PATH health, snapshot diffs, restore-script export, and the tool catalog.

## Goals

- Make workstation backup and migration simple for users who do not want to manage a GitHub repository.
- Keep all snapshot data portable and reviewable.
- Provide a public tool catalog with install commands and a requested-tool workflow.
- Preserve the existing GitHub sync path as an alternative remote.
- Keep the platform deployable as open-source software for self-hosting.

## Non-Goals

- Replacing local snapshots in `%USERPROFILE%\.odin`.
- Removing GitHub sync.
- Running restore operations from the browser (we export a script; the CLI restores).
- Storing raw API tokens, GitHub tokens, shell secrets, or credential values in plaintext.
- Multi-user teams, organizations, billing, or role-based permissions.

## Current State

The CLI supports local snapshot, restore, diff, history, rollback, GitHub sync, archive import/export, diagnostics, package-manager discovery, and Asgard profiles.

The platform provides:

- Next.js 16 App Router with **Better Auth** (self-hosted email/password) — no external provider.
- Mongo/Mongoose models for API tokens, machines, snapshots, device codes, rate limits, user settings, catalog tools, and tool requests.
- `POST /api/ingest` for snapshot upload (Bearer token, schema-validated, size-capped, rate-limited).
- OAuth 2.0 device-authorization login (`/api/device/*`, `/activate`).
- Dashboard pages for overview, snapshots (+ detail, diff, export, delete), tools, profiles, health, config vault, machines, settings, and maintainer request review.
- Public tool catalog with a missing-tool request workflow.

## Authentication & Tokens

- **Browser:** Better Auth session cookie; `/dashboard` guarded by `proxy.ts`.
- **CLI:** Bearer tokens in the format `odin_<keyId>_<secret>`. Only a bcrypt hash and the public `keyId` are stored; validation is an O(1) lookup by `keyId`. Legacy `odin_<hex>` tokens keep working via a fallback scan.
- Rate limiting on device/ingest routes (Mongo-backed fixed window) and auth routes (Better Auth built-in).

## Data Model

- `ApiToken`: `userId`, `label`, `tokenHash`, `keyId?`, `lastUsedAt`, timestamps.
- `Machine`: `userId`, `hostname`, `osName`, `osVersion`, `username`, `lastSeenAt`, timestamps.
- `Snapshot`: `snapshotId`, `machineId`, `userId`, `capturedAt`, `tag`, `schemaVersion`, raw sections (Mixed), `lockSha256` (server-computed SHA-256), timestamps.
- `DeviceCode`, `RateLimit` (TTL-indexed), `UserSettings` (retention), `CatalogTool`, `ToolRequest`.

## Platform API

See `docs/api.md` for the full reference. Ingest contract:

```http
POST /api/ingest
Authorization: Bearer odin_<keyId>_<secret>
Content-Type: application/json
```

Payload sections match the local snapshot files: `machine`, `environment`, `packages`, `vscode`, `git`, `lock` (+ optional `profiles`, `tag`). Response: `{ "ok": true, "snapshotId": "..." }`.

Export/diff additions:
- `GET /api/snapshots/[id]/export` — restore-ready PowerShell script (ownership-checked).
- `GET /api/snapshots/diff?a=&b=` — structured diff of two owned snapshots.

## Security And Privacy

- Records are always scoped by `userId`; no cross-user access.
- API tokens shown once, stored only as hashes; CLI stores them in the OS credential store.
- Snapshot upload redacts shell secrets and credential values CLI-side; the Config Vault masks secret-looking values UI-side as a second layer.
- Export requires authenticated ownership checks. Public catalog endpoints never expose user snapshot data.

## Open Source And Portability

Odin Platform is self-hostable; the hosted version is a convenience layer, not lock-in. Local snapshots and GitHub sync keep working without the platform. Restore-script export and (future) archive import keep data Odin-compatible.
