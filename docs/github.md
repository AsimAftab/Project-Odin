# GitHub Sync

Interactive setup:

```powershell
odin config github
```

Non-interactive setup:

```powershell
$env:GITHUB_TOKEN = "ghp_..."
odin config github --repo https://github.com/OWNER/REPO.git --branch main --non-interactive
```

Sync snapshots:

```powershell
odin sync
```

Odin stores repository metadata in `~/.odin/config.yaml` and stores GitHub tokens in the OS credential store through `keyring`.
