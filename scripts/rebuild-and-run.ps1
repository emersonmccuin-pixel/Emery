# Emery Rebuild & Run Script
# Kills running instances, rebuilds from current HEAD, installs, and launches.
# Usage: powershell -File scripts/rebuild-and-run.ps1
#        powershell -File scripts/rebuild-and-run.ps1 -SkipClient   # supervisor + mcp only

param(
    [switch]$SkipClient
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

$installDir = Join-Path (Join-Path $env:USERPROFILE ".emery") "bin"

# --- Kill all running Emery processes ---
Write-Host "=== Stopping Emery ===" -ForegroundColor Yellow

# Try PID files first (stable + dev)
foreach ($pidPath in @(
    (Join-Path $installDir "stable.pid"),
    (Join-Path (Join-Path $env:LOCALAPPDATA "Emery-Dev") "dev.pid")
)) {
    if (Test-Path $pidPath) {
        $pids = Get-Content $pidPath | ConvertFrom-Json
        foreach ($p in @($pids.supervisor, $pids.client)) {
            if ($p) { Get-Process -Id $p -ErrorAction SilentlyContinue | Stop-Process -Force }
        }
        Remove-Item $pidPath -ErrorAction SilentlyContinue
    }
}

# Fallback: kill by name if any survived
Get-Process -Name "emery-supervisor","emery-client" -ErrorAction SilentlyContinue | Stop-Process -Force

# Wait for file locks to release
$retries = 0
$targetExe = Join-Path $root "target\release\emery-client.exe"
while ($retries -lt 5) {
    Start-Sleep -Seconds 1
    if (-not (Test-Path $targetExe)) { break }
    try {
        [IO.File]::Open($targetExe, 'Open', 'ReadWrite', 'None').Close()
        break
    } catch {
        $retries++
        if ($retries -ge 5) {
            Write-Host "Warning: client binary may still be locked, proceeding anyway" -ForegroundColor Yellow
        }
    }
}

# --- Build ---
$commitHash = (git rev-parse --short HEAD).Trim()
$commitMsg = (git log -1 --format="%s").Trim()
Write-Host "=== Building $commitHash ===" -ForegroundColor Cyan

cargo build --release -p emery-supervisor -p emery-mcp
if ($LASTEXITCODE -ne 0) { Write-Host "Supervisor build failed!" -ForegroundColor Red; exit 1 }

if (-not $SkipClient) {
    Set-Location "$root\apps\emery-client"
    npm install --silent 2>$null
    Set-Location $root

    cargo tauri build
    if ($LASTEXITCODE -ne 0) { Write-Host "Client build failed!" -ForegroundColor Red; exit 1 }
}

# --- Install ---
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

Copy-Item "$root\target\release\emery-supervisor.exe" "$installDir\emery-supervisor.exe" -Force
Copy-Item "$root\target\release\emery-mcp.exe" "$installDir\emery-mcp.exe" -Force
if (-not $SkipClient) {
    Copy-Item "$root\target\release\emery-client.exe" "$installDir\emery-client.exe" -Force
}

@{
    commit = $commitHash
    ref = "HEAD"
    message = $commitMsg
    installed_at = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
} | ConvertTo-Json | Set-Content -Path "$installDir\version.json"

Write-Host "=== Installed $commitHash ===" -ForegroundColor Green

# --- Launch ---
Write-Host "=== Launching ===" -ForegroundColor Green
$supProc = Start-Process "$installDir\emery-supervisor.exe" -PassThru
Start-Sleep -Seconds 2

$clientPid = $null
if (-not $SkipClient) {
    $clientProc = Start-Process "$installDir\emery-client.exe" -PassThru
    $clientPid = $clientProc.Id
}

$pidFile = Join-Path $installDir "stable.pid"
@{ supervisor = $supProc.Id; client = $clientPid } | ConvertTo-Json | Set-Content $pidFile

Write-Host ""
Write-Host "=== Emery $commitHash is running ===" -ForegroundColor Green
