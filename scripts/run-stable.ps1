# Emery Run Stable Script
# Launches the stable-installed supervisor + client from ~/.emery/bin/
# Usage: powershell -File scripts/run-stable.ps1
#        powershell -File scripts/run-stable.ps1 -SupervisorOnly

param(
    [switch]$SupervisorOnly
)

$ErrorActionPreference = "Stop"
$installDir = Join-Path $env:USERPROFILE ".emery" "bin"

# --- Verify installation exists ---
if (-not (Test-Path "$installDir\emery-supervisor.exe")) {
    Write-Host "No stable installation found at $installDir" -ForegroundColor Red
    Write-Host "Run: powershell -File scripts/install-stable.ps1" -ForegroundColor Yellow
    exit 1
}

# --- Show version info ---
if (Test-Path "$installDir\version.json") {
    $ver = Get-Content "$installDir\version.json" | ConvertFrom-Json
    Write-Host "=== Emery Stable ($($ver.commit) @ $($ver.ref)) ===" -ForegroundColor Cyan
    Write-Host "  Installed: $($ver.installed_at)" -ForegroundColor Gray
    Write-Host "  Commit:    $($ver.message)" -ForegroundColor Gray
}

# --- Kill any running instances ---
Write-Host "=== Stopping existing Emery processes ===" -ForegroundColor Yellow
Get-Process -Name "emery-supervisor","emery-client" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

# --- Launch supervisor ---
Write-Host "=== Launching stable supervisor ===" -ForegroundColor Green
Start-Process "$installDir\emery-supervisor.exe"
Start-Sleep -Seconds 2

# --- Launch client ---
if (-not $SupervisorOnly) {
    if (-not (Test-Path "$installDir\emery-client.exe")) {
        Write-Host "No stable client binary found. Running supervisor only." -ForegroundColor Yellow
        Write-Host "Use -SkipClient when installing, or reinstall with client." -ForegroundColor Gray
    } else {
        Write-Host "=== Launching stable client ===" -ForegroundColor Green
        Start-Process "$installDir\emery-client.exe"
    }
}

Write-Host "=== Emery stable is running ===" -ForegroundColor Green
