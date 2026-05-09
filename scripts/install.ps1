param(
    [ValidateSet("User", "Machine")]
    [string]$Scope = "User",
    [string]$InstallDir,
    [string]$Repository = "example/odin",
    [string]$Version = "latest",
    [string]$LocalBinary,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Step([string]$Message) {
    Write-Host "[step] $Message" -ForegroundColor Cyan
}

function Write-Ok([string]$Message) {
    Write-Host "[ ok ] $Message" -ForegroundColor Green
}

function Write-Warn([string]$Message) {
    Write-Host "[warn] $Message" -ForegroundColor Yellow
}

function Test-IsAdmin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-DefaultInstallDir([string]$ScopeValue) {
    if ($ScopeValue -eq "Machine") {
        return Join-Path $env:ProgramFiles "Odin"
    }
    return Join-Path $env:LOCALAPPDATA "Odin\bin"
}

function Get-PathEntries([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return @()
    }
    return $Value.Split(";") | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
}

function Normalize-Entry([string]$Value) {
    return $Value.Trim().TrimEnd("\").ToLowerInvariant()
}

function Add-PathEntry([string]$ScopeValue, [string]$Entry) {
    $current = [Environment]::GetEnvironmentVariable("Path", $ScopeValue)
    $entries = Get-PathEntries $current
    $normalizedEntry = Normalize-Entry $Entry
    $existing = $entries | Where-Object { (Normalize-Entry $_) -eq $normalizedEntry }
    if ($existing.Count -gt 0) {
        Write-Ok "PATH already contains $Entry ($ScopeValue)"
        return
    }
    $updated = @($Entry) + $entries
    [Environment]::SetEnvironmentVariable("Path", ($updated -join ";"), $ScopeValue)
    Write-Ok "Added $Entry to $ScopeValue PATH"
}

function Refresh-Environment {
    $signature = @"
using System;
using System.Runtime.InteropServices;
public static class Win32 {
  [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
  public static extern IntPtr SendMessageTimeout(
      IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
      uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
}
"@
    Add-Type -TypeDefinition $signature -ErrorAction SilentlyContinue | Out-Null
    [UIntPtr]$result = [UIntPtr]::Zero
    [Win32]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, "Environment", 2, 5000, [ref]$result) | Out-Null
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
}

function Ensure-OdinDirectories {
    $root = Join-Path $env:USERPROFILE ".odin"
    $dirs = @("snapshots", "logs", "cache", "temp", "plugins")
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    foreach ($name in $dirs) {
        New-Item -ItemType Directory -Force -Path (Join-Path $root $name) | Out-Null
    }
    $configPath = Join-Path $root "config.yaml"
    if (-not (Test-Path $configPath)) {
        @"
storage_dir: null
restore:
  package_managers:
    - winget
    - choco
    - scoop
  restore_user_environment: true
  restore_path: true
  restore_vscode_extensions: true
  restore_git_config: true
sync:
  branch: main
  remote: null
github:
  repository_url: null
  branch: main
  token_key: null
"@ | Set-Content -Encoding UTF8 -Path $configPath
    }
}

function Resolve-ReleaseUrl([string]$Repo, [string]$VersionTag, [string]$AssetName) {
    if ($VersionTag -eq "latest") {
        return "https://github.com/$Repo/releases/latest/download/$AssetName"
    }
    return "https://github.com/$Repo/releases/download/$VersionTag/$AssetName"
}

if ($Scope -eq "Machine" -and -not (Test-IsAdmin)) {
    throw "Machine installation requires an elevated PowerShell session (Run as Administrator)."
}

$resolvedInstallDir = if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    Get-DefaultInstallDir $Scope
} else {
    $InstallDir
}

Write-Step "Preparing installation directory"
New-Item -ItemType Directory -Force -Path $resolvedInstallDir | Out-Null
$targetExe = Join-Path $resolvedInstallDir "odin.exe"
$backupExe = "$targetExe.bak"
$tempExe = Join-Path $env:TEMP ("odin-install-" + [Guid]::NewGuid().ToString() + ".exe")

try {
    if (-not [string]::IsNullOrWhiteSpace($LocalBinary)) {
        Write-Step "Using local binary from $LocalBinary"
        if (-not (Test-Path $LocalBinary)) {
            throw "Local binary was not found at $LocalBinary"
        }
        Copy-Item $LocalBinary $tempExe -Force
    } else {
        $downloadUrl = Resolve-ReleaseUrl -Repo $Repository -VersionTag $Version -AssetName "odin.exe"
        Write-Step "Downloading odin.exe from $downloadUrl"
        Invoke-WebRequest -Uri $downloadUrl -OutFile $tempExe -UseBasicParsing
    }

    if (Test-Path $targetExe) {
        if (-not $Force) {
            Write-Warn "Existing installation detected at $targetExe (use -Force to overwrite anyway)."
        }
        Move-Item $targetExe $backupExe -Force
    }

    Write-Step "Installing odin.exe to $targetExe"
    Move-Item $tempExe $targetExe -Force

    Add-PathEntry -ScopeValue $Scope -Entry $resolvedInstallDir
    Ensure-OdinDirectories
    Refresh-Environment

    Write-Step "Verifying executable"
    & $targetExe --version | Out-Null

    if (Test-Path $backupExe) {
        Remove-Item $backupExe -Force
    }

    Write-Ok "Odin installed successfully."
    Write-Host "Run: odin init"
} catch {
    if (Test-Path $targetExe -and (Test-Path $backupExe)) {
        Move-Item $backupExe $targetExe -Force
    }
    if (Test-Path $tempExe) {
        Remove-Item $tempExe -Force -ErrorAction SilentlyContinue
    }
    throw
}
