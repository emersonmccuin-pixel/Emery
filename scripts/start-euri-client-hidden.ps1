param(
    [switch]$EnableDiagnostics
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$clientRoot = Join-Path $repoRoot "apps\euri-client"
$viteOut = Join-Path $repoRoot "vite-hidden.out.log"
$viteErr = Join-Path $repoRoot "vite-hidden.err.log"
$clientExe = Join-Path $repoRoot "target\debug\euri-client.exe"

if ($EnableDiagnostics) {
    $env:EURI_DEV_DIAGNOSTICS = "1"
}

Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue |
    ForEach-Object {
        Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue
    }

Get-Process -Name euri-client -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue

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
        cargo build -p euri-client | Out-Null
    } finally {
        Pop-Location
    }
}

$clientProc = Start-Process -FilePath $clientExe -WorkingDirectory $repoRoot -PassThru

[pscustomobject]@{
    VitePid = $viteProc.Id
    ClientPid = $clientProc.Id
    FrontendUrl = "http://127.0.0.1:1420"
    DiagnosticsEnabled = [bool]$EnableDiagnostics
}
