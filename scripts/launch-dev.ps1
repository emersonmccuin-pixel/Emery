# Emery Dev Launch Script
# Builds and launches a dev instance alongside the running stable instance.
# Uses a separate data dir but shares knowledge.db with stable.
# Usage: powershell -File scripts/launch-dev.ps1
#        powershell -File scripts/launch-dev.ps1 -SkipBuild       # relaunch without rebuilding
#        powershell -File scripts/launch-dev.ps1 -SupervisorOnly   # rebuild + launch supervisor only

param(
    [switch]$SkipBuild,
    [switch]$SupervisorOnly
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

$devDataDir = Join-Path $env:LOCALAPPDATA "Emery-Dev"
$prodKnowledgeDb = Join-Path (Join-Path $env:LOCALAPPDATA "Emery") "knowledge.db"

function Start-WithEnv($exePath) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exePath
    $psi.UseShellExecute = $false
    $psi.EnvironmentVariables["EMERY_APP_DATA_DIR"] = $devDataDir
    $psi.EnvironmentVariables["EMERY_KNOWLEDGE_DB"] = $prodKnowledgeDb
    $psi.EnvironmentVariables["EMERY_DEV_DIAGNOSTICS"] = "1"
    $proc = [System.Diagnostics.Process]::Start($psi)
    return $proc
}

# --- Ensure dev data dir exists ---
if (-not (Test-Path $devDataDir)) {
    New-Item -ItemType Directory -Path $devDataDir -Force | Out-Null
    Write-Host "Created $devDataDir" -ForegroundColor Gray
}

# --- Kill previous dev instance (by PID file) ---
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
}

# --- Build ---
if (-not $SkipBuild) {
    Write-Host "=== Building supervisor + emery-mcp (release) ===" -ForegroundColor Cyan
    cargo build --release -p emery-supervisor -p emery-mcp
    if ($LASTEXITCODE -ne 0) { Write-Host "Supervisor build failed!" -ForegroundColor Red; exit 1 }

    if (-not $SupervisorOnly) {
        Write-Host "=== Installing frontend deps ===" -ForegroundColor Cyan
        Set-Location "$root\apps\emery-client"
        npm install --silent 2>$null
        Set-Location $root

        Write-Host "=== Building Tauri client (release) ===" -ForegroundColor Cyan
        cargo tauri build
        if ($LASTEXITCODE -ne 0) { Write-Host "Client build failed!" -ForegroundColor Red; exit 1 }
    }
}

# --- Launch supervisor ---
Write-Host "=== Launching dev supervisor ===" -ForegroundColor Green
Write-Host "  Data dir:     $devDataDir" -ForegroundColor Gray
Write-Host "  Knowledge DB: $prodKnowledgeDb" -ForegroundColor Gray

$supProc = Start-WithEnv "$root\target\release\emery-supervisor.exe"
Start-Sleep -Seconds 2

$clientPid = $null

# --- Launch client ---
if (-not $SupervisorOnly) {
    $clientExe = "$root\target\release\emery-client.exe"
    if (-not (Test-Path $clientExe)) {
        Write-Host "No dev client binary found. Running supervisor only." -ForegroundColor Yellow
    } else {
        Write-Host "=== Launching dev client ===" -ForegroundColor Green
        $clientProc = Start-WithEnv $clientExe
        $clientPid = $clientProc.Id
    }
}

# --- Write PID file ---
@{ supervisor = $supProc.Id; client = $clientPid } | ConvertTo-Json | Set-Content $pidFile

Write-Host ""
Write-Host "=== Emery dev is running ===" -ForegroundColor Green
Write-Host "  Dev data:   $devDataDir" -ForegroundColor Gray
Write-Host "  Shared KB:  $prodKnowledgeDb" -ForegroundColor Gray
Write-Host ""
Write-Host "Stable instance is unaffected." -ForegroundColor Cyan
