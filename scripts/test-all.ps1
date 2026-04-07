# Emery Test Runner
# Runs the full regression suite from the repo root.
# Usage: powershell -File scripts/test-all.ps1
#        powershell -File scripts/test-all.ps1 -SkipRust
#        powershell -File scripts/test-all.ps1 -SkipFrontend

param(
    [switch]$SkipRust,
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

function Invoke-Step($label, [scriptblock]$command) {
    Write-Host "=== $label ===" -ForegroundColor Cyan
    & $command
    if ($LASTEXITCODE -ne 0) {
        Write-Host "$label failed." -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

if (-not $SkipRust) {
    Invoke-Step "Rust tests" {
        cargo test -p supervisor-core -p supervisor-ipc -p emery-client
    }
}

if (-not $SkipFrontend) {
    Set-Location "$root\apps\emery-client"
    Invoke-Step "Frontend tests" {
        npm test
    }
    Invoke-Step "Frontend build" {
        npm run build
    }
    Set-Location $root
}

Write-Host ""
Write-Host "=== All requested tests passed ===" -ForegroundColor Green
