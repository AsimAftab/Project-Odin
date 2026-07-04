# winget packaging

Odin's winget manifests are **not hand-maintained in this repo**. They are
generated and submitted to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs)
automatically on each GitHub Release by `.github/workflows/winget.yml`
(via the `winget-releaser` action), which computes the installer hash from the
published `odin.exe` asset.

- **First-time submission** (creating `AsimAftab.Odin` in winget-pkgs) is a
  one-time manual step: `wingetcreate new <release-asset-url>`.
- **Subsequent releases** are picked up by the workflow — no manual manifest
  edits, and no per-version manifest files committed here.

A previously committed `manifests/.../0.2.0/` tree was removed: it drifted from
the released version and was never the source of truth. See `docs/release.md`.
