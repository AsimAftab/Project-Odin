# Release Process

CI runs on pushes and pull requests:

```text
.github/workflows/ci.yml
```

Release builds run when a semantic version tag is pushed:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds `target/release/odin.exe`, packages `odin-windows-x64.zip`, creates a GitHub Release, and uploads both assets.

Release artifacts:

- `odin.exe`
- `odin-windows-x64.zip`
- `install.ps1`
- `uninstall.ps1`
- `bootstrap.ps1`
- `checksums.txt`

The release body includes install/init instructions and points to `checksums.txt` for SHA-256 verification. These artifacts are the base for future packaging in MSI, winget, Chocolatey, and Scoop pipelines.

## winget

winget manifests are generated per-release, not committed to this repo. After a
GitHub Release publishes, `.github/workflows/winget.yml` submits a manifest
update PR to `microsoft/winget-pkgs` (the installer hash is computed from the
published `odin.exe`). The first-ever submission is a manual `wingetcreate new`
against the release asset URL. See `winget/README.md`.
