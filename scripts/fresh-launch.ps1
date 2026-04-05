# EURI Fresh Launch Script
# Kills all EURI processes, rebuilds everything, launches fresh.
# Usage: powershell -File scripts/fresh-launch.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

Write-Host "=== Killing existing EURI processes ===" -ForegroundColor Yellow
Get-Process -Name "euri-supervisor","euri-client" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

Write-Host "=== Building supervisor (release) ===" -ForegroundColor Cyan
cargo build --release -p euri-supervisor
if ($LASTEXITCODE -ne 0) { Write-Host "Supervisor build failed!" -ForegroundColor Red; exit 1 }

Write-Host "=== Installing frontend deps ===" -ForegroundColor Cyan
Set-Location "$root\apps\euri-client"
npm install --silent 2>$null

Write-Host "=== Building Tauri client (debug) ===" -ForegroundColor Cyan
Set-Location $root
cargo tauri build --debug
if ($LASTEXITCODE -ne 0) { Write-Host "Client build failed!" -ForegroundColor Red; exit 1 }

Write-Host "=== Launching supervisor ===" -ForegroundColor Green
Start-Process "$root\target\release\euri-supervisor.exe"
Start-Sleep -Seconds 2

Write-Host "=== Launching client ===" -ForegroundColor Green
Start-Process "$root\target\debug\euri-client.exe"

Write-Host "=== EURI is running ===" -ForegroundColor Green
