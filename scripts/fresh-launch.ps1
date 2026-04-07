# Emery Fresh Launch Script
# Kills all Emery processes, rebuilds everything, launches fresh.
# Usage: powershell -File scripts/fresh-launch.ps1
#        powershell -File scripts/fresh-launch.ps1 -SkipTests

param(
    [switch]$SkipTests
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

# --- Kill previous dev instance (by PID file, preserves stable) ---
$devDataDir = Join-Path $env:LOCALAPPDATA "Emery-Dev"
$pidFile = Join-Path $devDataDir "dev.pid"
if (Test-Path $pidFile) {
    Write-Host "=== Stopping previous dev instance ===" -ForegroundColor Yellow
    $pids = Get-Content $pidFile | ConvertFrom-Json
    foreach ($p in @($pids.supervisor, $pids.client)) {
        if ($p) {
            Get-Process -Id $p -ErrorAction SilentlyContinue | Stop-Process -Force
        }
    }
    Remove-Item $pidFile
    Start-Sleep -Seconds 1
} else {
    Write-Host "=== No dev PID file found, killing all Emery processes ===" -ForegroundColor Yellow
    Get-Process -Name "emery-supervisor","emery-client" -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 1
}

if (-not $SkipTests) {
    Write-Host "=== Running regression suite ===" -ForegroundColor Cyan
    powershell -ExecutionPolicy Bypass -File "$root\scripts\test-all.ps1"
    if ($LASTEXITCODE -ne 0) { Write-Host "Regression suite failed!" -ForegroundColor Red; exit 1 }
}

Write-Host "=== Building supervisor + emery-mcp (release) ===" -ForegroundColor Cyan
cargo build --release -p emery-supervisor -p emery-mcp
if ($LASTEXITCODE -ne 0) { Write-Host "Supervisor build failed!" -ForegroundColor Red; exit 1 }

Write-Host "=== Installing frontend deps ===" -ForegroundColor Cyan
Set-Location "$root\apps\emery-client"
npm install --silent 2>$null

Write-Host "=== Building Tauri client (release) ===" -ForegroundColor Cyan
Set-Location $root
cargo tauri build
if ($LASTEXITCODE -ne 0) { Write-Host "Client build failed!" -ForegroundColor Red; exit 1 }

Write-Host "=== Launching supervisor ===" -ForegroundColor Green
$env:EMERY_DEV_DIAGNOSTICS = "1"
Start-Process "$root\target\release\emery-supervisor.exe"
Start-Sleep -Seconds 2

Write-Host "=== Launching client ===" -ForegroundColor Green
Start-Process "$root\target\release\emery-client.exe"

Write-Host "=== Emery is running (diagnostics enabled) ===" -ForegroundColor Green
