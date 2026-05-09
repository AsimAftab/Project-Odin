param(
    [ValidateSet("User", "Machine", "All")]
    [string]$Scope = "User",
    [string]$InstallDir,
    [switch]$KeepConfig
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Step([string]$Message) {
    Write-Host "[step] $Message" -ForegroundColor Cyan
}

function Write-Ok([string]$Message) {
    Write-Host "[ ok ] $Message" -ForegroundColor Green
}

function Test-IsAdmin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Normalize-Entry([string]$Value) {
    return $Value.Trim().TrimEnd("\").ToLowerInvariant()
}

function Get-PathEntries([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return @()
    }
    return $Value.Split(";") | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
}

function Remove-PathEntry([string]$ScopeValue, [string]$Entry) {
    $current = [Environment]::GetEnvironmentVariable("Path", $ScopeValue)
    $normalized = Normalize-Entry $Entry
    $updated = Get-PathEntries $current | Where-Object { (Normalize-Entry $_) -ne $normalized }
    [Environment]::SetEnvironmentVariable("Path", ($updated -join ";"), $ScopeValue)
}

function Refresh-Environment {
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
}

if (($Scope -eq "Machine" -or $Scope -eq "All") -and -not (Test-IsAdmin)) {
    throw "Machine uninstall requires an elevated PowerShell session (Run as Administrator)."
}

$userDir = Join-Path $env:LOCALAPPDATA "Odin\bin"
$machineDir = Join-Path $env:ProgramFiles "Odin"
$dirs = if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    switch ($Scope) {
        "User" { @($userDir) }
        "Machine" { @($machineDir) }
        default { @($userDir, $machineDir) }
    }
} else {
    @($InstallDir)
}

Write-Step "Removing binaries"
foreach ($dir in $dirs) {
    if (Test-Path $dir) {
        Remove-Item $dir -Recurse -Force
        Write-Ok "Removed $dir"
    }
}

Write-Step "Cleaning PATH"
if ($Scope -eq "User" -or $Scope -eq "All") {
    foreach ($dir in $dirs) {
        Remove-PathEntry -ScopeValue "User" -Entry $dir
    }
}
if ($Scope -eq "Machine" -or $Scope -eq "All") {
    foreach ($dir in $dirs) {
        Remove-PathEntry -ScopeValue "Machine" -Entry $dir
    }
}

if (-not $KeepConfig) {
    $configRoot = Join-Path $env:USERPROFILE ".odin"
    if (Test-Path $configRoot) {
        Remove-Item $configRoot -Recurse -Force
        Write-Ok "Removed $configRoot"
    }
}

Refresh-Environment
Write-Ok "Odin uninstall complete."
