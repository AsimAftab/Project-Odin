<!-- Mirrored in both repos (Odin-Platform and Project-Odin). Edit together. -->

# Odin Platform Task Plan

Status legend: ✅ done · 🟡 partial · ⬜ pending.

## Phase 1: Spec, Docs, And Public Landing — ✅

- ✅ Product spec + task plan under `docs/`.
- ✅ Public landing page at `/`; dashboard reachable at `/dashboard` for signed-in users.

## Phase 2: CLI Platform Connection — ✅

- ✅ `platform` config in the CLI config model; `odin config platform` for URL + token.
- ✅ Tokens stored via the OS credential store.
- ✅ Upload service posts the latest snapshot to `/api/ingest`; `odin snapshot --push` and `upload_on_snapshot`.
- ✅ Device-flow login (`odin login`).

## Phase 3: Platform Ingest Hardening — ✅

- ✅ Keyed token format `odin_<keyId>_<secret>`; validation queries by `keyId` (O(1)), legacy tokens fall back to a bounded scan.
- ✅ `lastUsedAt` recorded after successful validation.
- ✅ Snapshot payload validated (zod, `lib/ingest-schema.ts`) with a 2 MB size cap → `400`/`413`.
- ✅ Real `lockSha256` (server-computed SHA-256 over captured sections).
- ✅ Rate limiting on device/ingest routes (`lib/rate-limit.ts`) and auth routes (Better Auth built-in).

## Phase 4: Catalog And Tool Requests — ✅

- ✅ Public `/catalog` + `CatalogTool`/`ToolRequest` models, seeded lazily.
- ✅ Copyable install commands (winget/choco/scoop); authenticated request flow; maintainer review at `/dashboard/requests`.

## Phase 5: Export, Import, And Migration — 🟡

- ✅ Single-snapshot restore-script export (`/api/snapshots/[id]/export`).
- ✅ Snapshot diff (`/api/snapshots/diff`), delete, and per-machine retention.
- ⬜ Multi-snapshot export bundle.
- ⬜ Import endpoint for Odin archive bundles.
- ⬜ Platform→GitHub migration workflow.

## Phase 6: Quality, Deployment, And Maintenance — 🟡

- ✅ Platform unit tests (`bun test`: token format, catalog-util, redaction, snapshot diff, restore script, user-code).
- ✅ CI (`.github/workflows/ci.yml`): lint, typecheck, tests, build.
- ✅ Self-host docs (README + `docs/architecture.md`, `docs/api.md`) for Better Auth, MongoDB, env vars.
- ✅ `SECURITY.md` + contribution guide.
- 🟡 CLI tests for config serialization and upload behavior (in progress in the CLI repo).
- ⬜ Deeper integration tests for ingest/device-flow DB paths.
