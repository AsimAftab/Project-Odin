# Security Policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Report privately via
GitHub's [private vulnerability reporting](https://github.com/AsimAftab/Project-Odin/security/advisories/new)
or email **alushmkr@gmail.com** with steps to reproduce and impact. We aim to
acknowledge within a few days.

## How Odin handles secrets

- **Credential storage.** Platform and GitHub API tokens are stored in the
  Windows credential store (via the `keyring` crate), never in
  `~/.odin/config.yaml` — the config only holds a `token_key` reference.
- **Redaction before upload.** When a snapshot is uploaded to the Odin Platform,
  secret-looking environment values are masked (`services/redact.rs`). Detection
  is two-layered:
  - **By name:** variables containing `TOKEN`, `SECRET`, `PASSWORD`, `KEY`,
    `CREDENTIAL`, `AUTH`, `PAT`, etc.
  - **By value shape:** GitHub tokens (`ghp_`/`github_pat_`/…), OpenAI/Anthropic
    `sk-…` keys, AWS access-key ids (`AKIA…`), Slack tokens, JWTs, and PEM
    private-key blocks — so a secret stored under a benign name is still masked.
  - PowerShell profile content is scanned line-by-line and masked the same way.

  **Limits:** redaction is heuristic. It won't catch a secret that has no
  recognizable name *and* no recognizable value shape (e.g. a bare high-entropy
  string in an unrelated variable). Review what you upload if in doubt.
- **Local snapshots are never redacted** — only the uploaded copy is. Local
  files under `~/.odin` hold the full captured state; secure that directory as
  you would any file holding machine configuration.
- **Uploads are non-destructive.** A failed or rejected upload never modifies or
  deletes local snapshots.

## Scope

Odin runs commands on your machine (package installs during restore, `schtasks`
for scheduling, PowerShell for env changes). Review generated restore scripts
(`odin export`) before running them, and only pair the CLI with a platform URL
you trust.
