param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("dev", "build")]
  [string]$Mode
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
$cargoPath = if ($cargoCommand) {
  $cargoCommand.Source
} else {
  Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
}

if (-not (Test-Path $cargoPath)) {
  Write-Error "Cargo executable not found. Expected it at '$cargoPath'."
}

$cargoDir = Split-Path -Parent $cargoPath
$env:PATH = "$cargoDir;$env:PATH"
$tauriCmd = Join-Path $repoRoot "node_modules\.bin\tauri.cmd"

if (-not (Test-Path $tauriCmd)) {
  Write-Error "Local Tauri CLI not found at '$tauriCmd'. Run 'npm install' first."
}

Push-Location $repoRoot

try {
  $triple = "x86_64-pc-windows-msvc"
  $binDir = Join-Path $repoRoot "src-tauri\binaries"
  if (-not (Test-Path $binDir)) { New-Item -ItemType Directory -Path $binDir | Out-Null }

  # Ensure placeholder files exist so tauri-build validation passes during cargo build
  foreach ($bin in @("project-commander-supervisor", "project-commander-cli")) {
    $dest = Join-Path $binDir "$bin-$triple.exe"
    if (-not (Test-Path $dest)) {
      [System.IO.File]::Create($dest).Close()
    }
  }

  $cargoArgs = @(
    "build"
    "--manifest-path"
    "src-tauri/Cargo.toml"
  )

  if ($Mode -eq "build") {
    $cargoArgs += "--release"
  }

  $cargoArgs += @(
    "--bin"
    "project-commander-cli"
    "--bin"
    "project-commander-supervisor"
  )

  & $cargoPath @cargoArgs

  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }

  # Copy real helper binaries over placeholders for Tauri externalBin bundling
  $buildProfile = if ($Mode -eq "build") { "release" } else { "debug" }
  $targetDir = Join-Path $repoRoot "src-tauri\target\$buildProfile"

  foreach ($bin in @("project-commander-supervisor", "project-commander-cli")) {
    Copy-Item (Join-Path $targetDir "$bin.exe") (Join-Path $binDir "$bin-$triple.exe") -Force
  }

  & $tauriCmd $Mode

  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
} finally {
  Pop-Location
}
