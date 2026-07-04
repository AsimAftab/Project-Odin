# Odin Platform Product Spec

## Summary

Odin Platform is the account-based hub for backing up, reviewing, restoring, and migrating developer workstation state captured by the open-source Odin CLI. The CLI remains the local agent and portable source of truth; the platform makes that state easier to manage across machines without forcing users into GitHub as the only remote.

The first platform experience is:

1. A user creates an Odin Platform account.
2. The user installs Odin with `winget install AsimAftab.Odin`.
3. The user generates a platform API token in the dashboard.
4. The user connects the CLI to the platform.
5. `odin snapshot` captures local workstation state and uploads it to the user's account.
6. The dashboard shows machines, snapshots, packages, VS Code extensions, Git config, PATH health, and migration/export options.

## Goals

- Make workstation backup and migration simple for users who do not want to create and manage a GitHub repository.
- Keep all snapshot data portable and reviewable.
- Provide a public tool catalog with install commands, supported package managers, and requested-tool workflow.
- Preserve the existing GitHub sync path as an alternative remote.
- Keep the platform deployable as open-source software for self-hosting.

## Non-Goals For The First Release

- Replacing local snapshots in `%USERPROFILE%\.odin`.
- Removing GitHub sync.
- Running restore operations from the browser.
- Storing raw API tokens, GitHub tokens, shell secrets, or credential values in plaintext.
- Supporting multi-user teams, organizations, billing, or role-based permissions.

## Existing State

The CLI project already supports local snapshot, restore, diff, history, rollback, GitHub sync, archive import/export, diagnostics, package-manager discovery, and Asgard profiles.

The platform app already has:

- Next.js app router with Clerk authentication.
- Mongo/Mongoose models for API tokens, machines, and snapshots.
- `POST /api/ingest` for CLI-style snapshot upload using bearer tokens.
- Dashboard pages for overview, snapshots, tools, profiles, health, config, machines, and settings.
- API token generation from the settings page.

Current gaps:

- `/` redirects to auth instead of presenting a public landing page.
- The CLI does not yet have a platform config/login/upload command.
- Platform docs mention `odin config push`, but the CLI does not implement it.
- API token validation scans all tokens and bcrypt-compares each one, which should be fixed before scale.
- The platform has dashboard views but no public catalog or tool-request workflow.
- Export/migration is not yet available from the platform UI or API.

## User Flows

### Public Visitor

- Lands on `/`.
- Sees Odin Platform as an open-source workstation backup and migration hub.
- Copies install command: `winget install AsimAftab.Odin`.
- Can open sign-in/sign-up.
- Can view the product promise: snapshots, machine timeline, config vault, tool catalog, and export/migration.

### New User

- Signs up with Clerk.
- Opens dashboard settings.
- Generates an API token labeled with the machine name.
- Runs the platform CLI setup command.
- Runs `odin snapshot`.
- Sees the machine and first snapshot in the dashboard.

### Returning User

- Opens the dashboard and checks latest machine state.
- Reviews packages and tools by package manager.
- Checks PATH and missing-tool findings.
- Opens a snapshot detail page.
- Exports or migrates snapshot data when that phase is implemented.

### Migration User

- Selects one or more snapshots.
- Exports an archive compatible with Odin restore/import.
- Optionally pushes snapshot data to a GitHub repo.
- Uses Odin locally to restore on another machine.

### Catalog User

- Opens the public catalog.
- Searches for a tool.
- Sees install commands for `winget`, Chocolatey, Scoop, direct installer, and docs where available.
- Requests a missing tool if not found.

## Data Model

Use the current platform models as the first baseline:

- `ApiToken`: `userId`, `label`, `tokenHash`, `lastUsedAt`, timestamps.
- `Machine`: `userId`, `hostname`, `osName`, `osVersion`, `username`, `lastSeenAt`, timestamps.
- `Snapshot`: `snapshotId`, `machineId`, `userId`, `capturedAt`, `tag`, `schemaVersion`, raw snapshot sections, `lockSha256`, timestamps.

Near-term model improvements:

- Add a token lookup prefix or token id to avoid scanning every token during ingest.
- Store a real integrity hash for the uploaded lock or payload instead of using `snapshot_id` as `lockSha256`.
- Add optional fields for platform upload metadata: CLI version, platform endpoint, upload time, and ingest schema version.

Future catalog models:

- `Tool`: slug, name, description, homepage, categories, supported platforms, install commands, package ids, aliases, maintained status.
- `ToolRequest`: userId, requested name, package manager hints, notes, status, timestamps.

## CLI Integration

Add platform config beside the existing GitHub config. The platform feature must be opt-in and must not break local snapshot or GitHub sync behavior.

Recommended command shape:

```powershell
odin config platform --url https://odin.example.com --token odin_xxx
odin snapshot --push
```

Interactive mode should prompt for URL and token when omitted.

Config additions:

```yaml
platform:
  url: "https://odin.example.com"
  token_key: "odin-platform:https://odin.example.com"
  upload_on_snapshot: false
```

Behavior:

- Store the API token using the existing OS credential-store pattern.
- `odin snapshot --push` uploads the captured snapshot to `POST /api/ingest`.
- If `upload_on_snapshot` is true, `odin snapshot` uploads automatically after local snapshot succeeds.
- Upload failures should not delete or corrupt local snapshots.
- A failed upload should return a clear error and leave the user with a retry path.

## Platform API

Current ingest endpoint:

```http
POST /api/ingest
Authorization: Bearer odin_xxx
Content-Type: application/json
```

Payload sections should match the local snapshot files:

- `machine`
- `environment`
- `packages`
- `vscode`
- `git`
- `lock`

Expected response:

```json
{ "ok": true, "snapshotId": "..." }
```

Near-term API additions:

- `GET /api/export/snapshots/:id` for restore-ready snapshot export.
- `GET /api/catalog` for public catalog search.
- `POST /api/tool-requests` for authenticated missing-tool requests.

## Landing Page

Route `/` becomes public. Signed-in users can still reach `/dashboard` through a visible dashboard link.

Required first-viewport signals:

- Product name: `Odin Platform`.
- Open-source positioning.
- Install command: `winget install AsimAftab.Odin`.
- Clear sign-up/sign-in calls to action.
- Visual representation of machines, snapshots, tool catalog, and migration.

Design direction:

- Industrial and operational, suited to a developer tooling product.
- Dark interface consistent with the existing dashboard.
- Avoid generic SaaS hero cards and vague gradients.
- Use concrete command snippets and product data, not abstract marketing copy.

## Security And Privacy

- Platform records are always scoped by Clerk `userId`.
- API tokens are shown once and stored only as hashes.
- CLI tokens are stored in the OS credential store.
- Snapshot upload should avoid shell secrets and credential values.
- Export must require authenticated ownership checks.
- Public catalog endpoints must not expose user snapshot data.

## Open Source And Portability

Odin Platform should be self-hostable. The hosted version is a convenience layer, not a lock-in boundary.

Portability requirements:

- Users can keep using local snapshots without the platform.
- Users can keep syncing to GitHub.
- Platform export returns Odin-compatible snapshot data.
- Future imports should accept Odin archives created by `odin archive export`.

## Acceptance Criteria

- The repository contains this spec and a task-management doc.
- The platform root route is public and presents Odin Platform clearly.
- Existing dashboard routes continue to require auth.
- The landing page includes install, account, snapshot, catalog, and migration messaging.
- The next implementation phase can start without redefining product scope.
