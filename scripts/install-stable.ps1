# Emery Stable Install Script
# Builds supervisor + client from a git ref and installs to ~/.emery/bin/
# Usage:
#   powershell -File scripts/install-stable.ps1              # build from current HEAD
#   powershell -File scripts/install-stable.ps1 -Ref main    # build from main branch
#   powershell -File scripts/install-stable.ps1 -Ref v0.2.0  # build from a tag

param(
    [string]$Ref = "",
    [switch]$SkipClient
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

$installDir = Join-Path $env:USERPROFILE ".emery" "bin"

# --- Ensure install directory exists ---
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Write-Host "Created $installDir" -ForegroundColor Gray
}

# --- Optionally checkout a ref ---
$stashed = $false
$originalBranch = ""
if ($Ref -ne "") {
    $originalBranch = (git rev-parse --abbrev-ref HEAD).Trim()
    Write-Host "=== Stashing local changes ===" -ForegroundColor Yellow
    $stashResult = git stash push -m "install-stable auto-stash" 2>&1
    if ($stashResult -notmatch "No local changes") { $stashed = $true }

    Write-Host "=== Checking out $Ref ===" -ForegroundColor Yellow
    git checkout $Ref
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to checkout $Ref" -ForegroundColor Red
        if ($stashed) { git stash pop }
        exit 1
    }
}

$commitHash = (git rev-parse --short HEAD).Trim()
$commitMsg = (git log -1 --format="%s").Trim()
Write-Host "=== Building from $commitHash ($commitMsg) ===" -ForegroundColor Cyan

# --- Build supervisor (release) ---
Write-Host "=== Building supervisor (release) ===" -ForegroundColor Cyan
cargo build --release -p emery-supervisor
if ($LASTEXITCODE -ne 0) {
    Write-Host "Supervisor build failed!" -ForegroundColor Red
    if ($originalBranch -ne "") { git checkout $originalBranch; if ($stashed) { git stash pop } }
    exit 1
}

# --- Build client (release) ---
if (-not $SkipClient) {
    Write-Host "=== Installing frontend deps ===" -ForegroundColor Cyan
    Set-Location "$root\apps\emery-client"
    npm install --silent 2>$null
    Set-Location $root

    Write-Host "=== Building Tauri client (release) ===" -ForegroundColor Cyan
    cargo tauri build
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Client build failed!" -ForegroundColor Red
        if ($originalBranch -ne "") { git checkout $originalBranch; if ($stashed) { git stash pop } }
        exit 1
    }
}

# --- Copy binaries ---
Write-Host "=== Installing to $installDir ===" -ForegroundColor Green

Copy-Item "$root\target\release\emery-supervisor.exe" "$installDir\emery-supervisor.exe" -Force
Write-Host "  emery-supervisor.exe  -> $installDir" -ForegroundColor Gray

if (-not $SkipClient) {
    # Tauri release build outputs to target/release
    Copy-Item "$root\target\release\emery-client.exe" "$installDir\emery-client.exe" -Force
    Write-Host "  emery-client.exe      -> $installDir" -ForegroundColor Gray
}

# --- Write version marker ---
$versionInfo = @{
    commit = $commitHash
    ref = if ($Ref -ne "") { $Ref } else { "HEAD" }
    message = $commitMsg
    installed_at = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
} | ConvertTo-Json
Set-Content -Path "$installDir\version.json" -Value $versionInfo
Write-Host "  version.json         -> $installDir" -ForegroundColor Gray

# --- Restore original branch ---
if ($originalBranch -ne "") {
    Write-Host "=== Restoring $originalBranch ===" -ForegroundColor Yellow
    git checkout $originalBranch
    if ($stashed) { git stash pop }
}

Write-Host ""
Write-Host "=== Stable install complete ===" -ForegroundColor Green
Write-Host "  Commit:  $commitHash" -ForegroundColor Gray
Write-Host "  Path:    $installDir" -ForegroundColor Gray
Write-Host ""
Write-Host "Run with: powershell -File scripts/run-stable.ps1" -ForegroundColor Cyan
