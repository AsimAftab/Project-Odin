param(
    [ValidateSet("User", "Machine")]
    [string]$Scope = "User",
    [string]$Repository = "example/odin",
    [string]$Version = "latest",
    [string]$InstallDir
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-InstallScriptUrl([string]$Repo, [string]$VersionTag) {
    if ($VersionTag -eq "latest") {
        return "https://github.com/$Repo/releases/latest/download/install.ps1"
    }
    return "https://github.com/$Repo/releases/download/$VersionTag/install.ps1"
}

$url = Resolve-InstallScriptUrl -Repo $Repository -VersionTag $Version
$tempFile = Join-Path $env:TEMP ("odin-bootstrap-install-" + [Guid]::NewGuid().ToString() + ".ps1")

try {
    Invoke-WebRequest -Uri $url -OutFile $tempFile -UseBasicParsing
    & powershell -NoProfile -ExecutionPolicy Bypass -File $tempFile -Scope $Scope -Repository $Repository -Version $Version -InstallDir $InstallDir
} finally {
    if (Test-Path $tempFile) {
        Remove-Item $tempFile -Force -ErrorAction SilentlyContinue
    }
}
