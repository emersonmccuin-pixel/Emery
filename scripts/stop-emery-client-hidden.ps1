$ErrorActionPreference = "SilentlyContinue"

Get-Process -Name emery-client | Stop-Process -Force

Get-NetTCPConnection -LocalPort 1420 -State Listen |
    ForEach-Object {
        Stop-Process -Id $_.OwningProcess -Force
    }
