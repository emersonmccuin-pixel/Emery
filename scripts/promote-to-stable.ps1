# Emery Promote to Stable
# Copies the current dev build to the stable install dir (~/.emery/bin/).
# Run this after testing a dev build you're happy with.
# Usage: powershell -File scripts/promote-to-stable.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$installDir = Join-Path (Join-Path $env:USERPROFILE ".emery") "bin"

# --- Verify dev binaries exist ---
$supSrc = "$root\target\release\emery-supervisor.exe"
$clientSrc = "$root\target\debug\emery-client.exe"
$clientReleaseSrc = "$root\target\release\emery-client.exe"

if (-not (Test-Path $supSrc)) {
    Write-Host "No dev supervisor build found at $supSrc" -ForegroundColor Red
    Write-Host "Run: powershell -File scripts/launch-dev.ps1" -ForegroundColor Yellow
    exit 1
}

# Prefer release client if available, fall back to debug
$clientToCopy = if (Test-Path $clientReleaseSrc) { $clientReleaseSrc } elseif (Test-Path $clientSrc) { $clientSrc } else { $null }

# --- Confirm ---
$commitHash = (git rev-parse --short HEAD).Trim()
$commitMsg = (git log -1 --format="%s").Trim()
Write-Host "=== Promoting dev build to stable ===" -ForegroundColor Cyan
Write-Host "  Commit:  $commitHash ($commitMsg)" -ForegroundColor Gray
Write-Host "  From:    $root\target\" -ForegroundColor Gray
Write-Host "  To:      $installDir" -ForegroundColor Gray
Write-Host ""

# --- Stop stable instance ---
$stablePidFile = Join-Path $installDir "stable.pid"
if (Test-Path $stablePidFile) {
    Write-Host "=== Stopping stable instance ===" -ForegroundColor Yellow
    $pids = Get-Content $stablePidFile | ConvertFrom-Json
    foreach ($p in @($pids.supervisor, $pids.client)) {
        if ($p) {
            Get-Process -Id $p -ErrorAction SilentlyContinue | Stop-Process -Force
        }
    }
    Remove-Item $stablePidFile
    Start-Sleep -Seconds 1
}

# --- Ensure install dir exists ---
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

# --- Copy binaries ---
Copy-Item $supSrc "$installDir\emery-supervisor.exe" -Force
Write-Host "  emery-supervisor.exe  -> $installDir" -ForegroundColor Gray

if ($clientToCopy) {
    Copy-Item $clientToCopy "$installDir\emery-client.exe" -Force
    Write-Host "  emery-client.exe      -> $installDir" -ForegroundColor Gray
} else {
    Write-Host "  No client binary found, skipping" -ForegroundColor Yellow
}

# --- Write version marker ---
@{
    commit = $commitHash
    ref = "dev (promoted)"
    message = $commitMsg
    installed_at = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
} | ConvertTo-Json | Set-Content "$installDir\version.json"

Write-Host ""
Write-Host "=== Promoted to stable ===" -ForegroundColor Green
Write-Host "Run: powershell -File scripts/run-stable.ps1" -ForegroundColor Cyan
