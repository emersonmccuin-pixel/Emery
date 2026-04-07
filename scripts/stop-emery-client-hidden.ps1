$ErrorActionPreference = "SilentlyContinue"

$repoRoot = Split-Path -Parent $PSScriptRoot
$pidFile = Join-Path $repoRoot ".emery-hidden-dev-pids.json"

function Stop-RecordedProcess {
    param([int]$Id)

    if ($Id -le 0) {
        return
    }

    try {
        Stop-Process -Id $Id -Force -ErrorAction SilentlyContinue
    } catch {
    }
}

if (Test-Path $pidFile) {
    try {
        $recorded = Get-Content $pidFile -Raw | ConvertFrom-Json
        if ($recorded.clientPid) {
            Stop-RecordedProcess -Id ([int]$recorded.clientPid)
        }
        if ($recorded.vitePid) {
            Stop-RecordedProcess -Id ([int]$recorded.vitePid)
        }
    } catch {
    }
    Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
}
