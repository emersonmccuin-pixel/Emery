# Emery Fresh Launch Script
# Kills all Emery processes, rebuilds everything, launches fresh.
# Usage: powershell -File scripts/fresh-launch.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

Write-Host "=== Killing existing Emery processes ===" -ForegroundColor Yellow
Get-Process -Name "emery-supervisor","emery-client" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

Write-Host "=== Building supervisor (release) ===" -ForegroundColor Cyan
cargo build --release -p emery-supervisor
if ($LASTEXITCODE -ne 0) { Write-Host "Supervisor build failed!" -ForegroundColor Red; exit 1 }

Write-Host "=== Installing frontend deps ===" -ForegroundColor Cyan
Set-Location "$root\apps\emery-client"
npm install --silent 2>$null

Write-Host "=== Building Tauri client (debug) ===" -ForegroundColor Cyan
Set-Location $root
cargo tauri build --debug
if ($LASTEXITCODE -ne 0) { Write-Host "Client build failed!" -ForegroundColor Red; exit 1 }

Write-Host "=== Launching supervisor ===" -ForegroundColor Green
Start-Process "$root\target\release\emery-supervisor.exe"
Start-Sleep -Seconds 2

Write-Host "=== Launching client ===" -ForegroundColor Green
Start-Process "$root\target\debug\emery-client.exe"

Write-Host "=== Emery is running ===" -ForegroundColor Green
