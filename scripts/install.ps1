$ErrorActionPreference = 'Stop'

$InstallDir = Join-Path $env:LOCALAPPDATA 'Odin\bin'
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$Source = Join-Path $PSScriptRoot '..\target\release\odin.exe'
if (-not (Test-Path $Source)) {
    throw "Release binary not found at $Source. Run cargo build --release first."
}

Copy-Item $Source (Join-Path $InstallDir 'odin.exe') -Force

$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($UserPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable('Path', "$InstallDir;$UserPath", 'User')
}

Write-Host "Installed odin.exe to $InstallDir"
Write-Host "Open a new terminal and run: odin --help"
