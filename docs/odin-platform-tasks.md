# Odin Platform Task Plan

## Phase 1: Spec, Docs, And Public Landing

- Add product spec in `docs/odin-platform-spec.md`.
- Add this task plan in `docs/odin-platform-tasks.md`.
- Replace `odin-platform/app/page.tsx` auth redirect with a public landing page.
- Keep dashboard access available through `/dashboard` for signed-in users.
- Update platform metadata if needed to describe the open-source workstation backup hub.
- Verify platform lint/build.

Acceptance:

- `/` can be viewed without signing in.
- Landing page has CTAs for install, sign up, sign in, and dashboard.
- Landing page references snapshots, migration/export, and catalog.

## Phase 2: CLI Platform Connection

- Add `platform` config to the CLI config model.
- Add `odin config platform` for platform URL and API token setup.
- Store platform API tokens using the existing credential-store service.
- Add a platform upload service that posts the latest snapshot payload to `/api/ingest`.
- Add `odin snapshot --push` and optional `upload_on_snapshot` config behavior.
- Document the workflow in `docs/usage.md`.

Acceptance:

- A user can connect the CLI to a platform URL.
- A user can upload a snapshot after local capture.
- Upload failure does not remove or modify local snapshot files.
- Existing GitHub sync still works.

## Phase 3: Platform Ingest Hardening

- Add a token prefix or lookup key to `ApiToken`.
- Update token generation to return tokens with a lookup prefix.
- Update validation to query candidate token records before bcrypt comparison.
- Record `lastUsedAt` after successful validation.
- Validate snapshot payload shape before writing Mongo records.
- Store a real payload or lock hash for `lockSha256`.

Acceptance:

- Token validation is bounded to a small candidate set.
- Invalid payloads return `400` with useful errors.
- Invalid tokens return `401`.
- Existing tokens either continue to work through a compatibility path or are intentionally reset with a migration note.

## Phase 4: Catalog And Tool Requests

- Add a public `/catalog` route.
- Add catalog data model for tools and install commands.
- Seed initial tools from current Odin-supported package managers and common developer runtimes.
- Show install commands for `winget`, Chocolatey, Scoop, direct installer, and docs where known.
- Add authenticated missing-tool request flow.
- Add dashboard view for a user's requests.

Acceptance:

- Users can search for tools.
- Each tool page shows copyable install commands.
- Users can request new tools.
- Maintainers can review requested tools through data records or an admin-ready workflow.

## Phase 5: Export, Import, And Migration

- Add snapshot export API for a single snapshot.
- Add multi-snapshot export bundle later if needed.
- Add dashboard export buttons on snapshot pages.
- Add GitHub migration workflow from platform to repository.
- Add import endpoint for Odin archive bundles after CLI archive compatibility is confirmed.

Acceptance:

- A user can download their own snapshot data.
- Exported data can be restored or imported by Odin.
- Users cannot access exports belonging to another account.
- GitHub remains an optional target.

## Phase 6: Quality, Deployment, And Maintenance

- Add tests for platform API auth and ingest paths.
- Add CLI tests for config serialization and upload behavior.
- Add deployment/self-host docs for Clerk, MongoDB, and environment variables.
- Add privacy/security notes for snapshot content.
- Add contribution guide for catalog tool entries.

Acceptance:

- Platform build and lint pass.
- CLI `cargo test` passes.
- A new contributor can run both projects locally.
- A self-hosting user can configure auth, database, and platform URL.
