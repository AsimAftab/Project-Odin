# Odin Architecture

Odin is organized around commands, services, strongly typed models, platform integrations, and terminal UI.

```text
src/
  commands/       CLI command handlers
  core/           application context and typed errors
  integrations/   Windows tools, Git, GitHub, PowerShell, VS Code
  models/         serde-compatible snapshot and config types
  services/       snapshot, restore, sync, config, secrets, storage
  ui/             Ratatui dashboard views
  utils/          filesystem, logging, checksums, terminal helpers
```

Windows integration is isolated under `integrations/` so Linux and macOS discovery can be added later without changing snapshot storage models.
