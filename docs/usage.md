# Usage

Initialize Odin:

```powershell
odin init
```

Install globally (per-user):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\bootstrap.ps1 -Repository OWNER/REPO -Scope User
```

Capture the workstation:

```powershell
odin snapshot
```

Open the dashboard:

```powershell
odin dashboard
```

Diagnose machine health:

```powershell
odin doctor
```

Check for updates:

```powershell
odin update --check
```

Install the latest available update:

```powershell
odin update
```

Preview restore:

```powershell
odin restore
```

Apply restore:

```powershell
odin restore --apply
```

Backup snapshots to GitHub:

```powershell
odin sync
```

`odin backup` is an alias for `odin sync`.
