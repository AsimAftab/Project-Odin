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
