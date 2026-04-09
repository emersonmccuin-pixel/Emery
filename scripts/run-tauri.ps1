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
    "project-commander-mcp"
    "--bin"
    "project-commander-supervisor"
  )

  & $cargoPath @cargoArgs

  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }

  & $tauriCmd $Mode

  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
} finally {
  Pop-Location
}
