param(
    [switch]$EnableDiagnostics
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$clientRoot = Join-Path $repoRoot "apps\emery-client"
$viteOut = Join-Path $repoRoot "vite-hidden.out.log"
$viteErr = Join-Path $repoRoot "vite-hidden.err.log"
$clientExe = Join-Path $repoRoot "target\debug\emery-client.exe"
$pidFile = Join-Path $repoRoot ".emery-hidden-dev-pids.json"

if ($EnableDiagnostics) {
    $env:EMERY_DEV_DIAGNOSTICS = "1"
}

function Stop-RecordedProcess {
    param(
        [string]$Name,
        [int]$Id
    )

    if ($Id -le 0) {
        return
    }

    try {
        $proc = Get-Process -Id $Id -ErrorAction Stop
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    } catch {
        Write-Verbose "No recorded $Name process found for PID $Id"
    }
}

if (Test-Path $pidFile) {
    try {
        $recorded = Get-Content $pidFile -Raw | ConvertFrom-Json
        if ($recorded.vitePid) {
            Stop-RecordedProcess -Name "vite" -Id ([int]$recorded.vitePid)
        }
        if ($recorded.clientPid) {
            Stop-RecordedProcess -Name "client" -Id ([int]$recorded.clientPid)
        }
    } catch {
        Write-Warning "Failed to parse $pidFile. Continuing without recorded PID cleanup."
    }
    Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
}

$portOwner = Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue |
    Select-Object -First 1
if ($portOwner) {
    throw "Port 1420 is already in use by PID $($portOwner.OwningProcess). Refusing to kill an arbitrary process."
}

foreach ($path in @($viteOut, $viteErr)) {
    if (Test-Path $path) {
        Remove-Item $path -Force
    }
}

$viteProc = Start-Process `
    -FilePath "node.exe" `
    -ArgumentList "node_modules/vite/bin/vite.js", "--host", "127.0.0.1", "--clearScreen", "false" `
    -WorkingDirectory $clientRoot `
    -RedirectStandardOutput $viteOut `
    -RedirectStandardError $viteErr `
    -WindowStyle Hidden `
    -PassThru

$deadline = (Get-Date).AddSeconds(20)
$ready = $false
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 250
    try {
        $response = Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:1420" -TimeoutSec 2
        if ($response.StatusCode -eq 200) {
            $ready = $true
            break
        }
    } catch {
    }
}

if (-not $ready) {
    Stop-Process -Id $viteProc.Id -Force -ErrorAction SilentlyContinue
    throw "Vite dev server did not become ready on http://127.0.0.1:1420"
}

if (-not (Test-Path $clientExe)) {
    Push-Location $repoRoot
    try {
        cargo build -p emery-client | Out-Null
    } finally {
        Pop-Location
    }
}

$clientProc = Start-Process -FilePath $clientExe -WorkingDirectory $repoRoot -PassThru

@{
    vitePid = $viteProc.Id
    clientPid = $clientProc.Id
} | ConvertTo-Json | Set-Content -Path $pidFile

[pscustomobject]@{
    VitePid = $viteProc.Id
    ClientPid = $clientProc.Id
    FrontendUrl = "http://127.0.0.1:1420"
    DiagnosticsEnabled = [bool]$EnableDiagnostics
}
