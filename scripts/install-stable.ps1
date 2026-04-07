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

$installDir = Join-Path (Join-Path $env:USERPROFILE ".emery") "bin"
$buildRoot = Join-Path $env:TEMP ("emery-stable-" + [guid]::NewGuid().ToString("N"))
$resolvedRef = if ($Ref -ne "") { $Ref } else { "HEAD" }

# --- Ensure install directory exists ---
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Write-Host "Created $installDir" -ForegroundColor Gray
}

try {
    Write-Host "=== Creating detached build worktree at $buildRoot ===" -ForegroundColor Yellow
    git worktree add --detach $buildRoot $resolvedRef
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create build worktree for ref '$resolvedRef'"
    }

    $commitHash = (git -C $buildRoot rev-parse --short HEAD).Trim()
    $commitMsg = (git -C $buildRoot log -1 --format="%s").Trim()
    Write-Host "=== Building from $commitHash ($commitMsg) ===" -ForegroundColor Cyan

    # --- Build supervisor + MCP server (release) ---
    Write-Host "=== Building supervisor + emery-mcp (release) ===" -ForegroundColor Cyan
    cargo build --release -p emery-supervisor -p emery-mcp --manifest-path "$buildRoot\Cargo.toml"
    if ($LASTEXITCODE -ne 0) {
        throw "Supervisor build failed!"
    }

    # --- Build client (release) ---
    if (-not $SkipClient) {
        Write-Host "=== Installing frontend deps ===" -ForegroundColor Cyan
        Push-Location "$buildRoot\apps\emery-client"
        try {
            npm install --silent 2>$null
        } finally {
            Pop-Location
        }

        Write-Host "=== Building Tauri client (release) ===" -ForegroundColor Cyan
        Push-Location $buildRoot
        try {
            cargo tauri build
        } finally {
            Pop-Location
        }
        if ($LASTEXITCODE -ne 0) {
            throw "Client build failed!"
        }
    }

    # --- Copy binaries ---
    Write-Host "=== Installing to $installDir ===" -ForegroundColor Green

    Copy-Item "$buildRoot\target\release\emery-supervisor.exe" "$installDir\emery-supervisor.exe" -Force
    Write-Host "  emery-supervisor.exe  -> $installDir" -ForegroundColor Gray

    Copy-Item "$buildRoot\target\release\emery-mcp.exe" "$installDir\emery-mcp.exe" -Force
    Write-Host "  emery-mcp.exe         -> $installDir" -ForegroundColor Gray

    if (-not $SkipClient) {
        Copy-Item "$buildRoot\target\release\emery-client.exe" "$installDir\emery-client.exe" -Force
        Write-Host "  emery-client.exe      -> $installDir" -ForegroundColor Gray
    }

    # --- Write version marker ---
    $versionInfo = @{
        commit = $commitHash
        ref = $resolvedRef
        message = $commitMsg
        installed_at = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    } | ConvertTo-Json
    Set-Content -Path "$installDir\version.json" -Value $versionInfo
    Write-Host "  version.json         -> $installDir" -ForegroundColor Gray

    Write-Host ""
    Write-Host "=== Stable install complete ===" -ForegroundColor Green
    Write-Host "  Commit:  $commitHash" -ForegroundColor Gray
    Write-Host "  Path:    $installDir" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Run with: powershell -File scripts/run-stable.ps1" -ForegroundColor Cyan
} finally {
    if (Test-Path $buildRoot) {
        git worktree remove --force $buildRoot | Out-Null
    }
}
