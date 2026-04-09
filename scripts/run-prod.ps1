$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $repoRoot "src-tauri\target\release\project-commander.exe"

if (-not (Test-Path $exePath)) {
  Write-Error "Production executable not found at '$exePath'. Run 'npm run prod:build' first."
}

Start-Process -FilePath $exePath
