Get-CimInstance Win32_Process -Filter "name='claude.exe'" |
    Sort-Object CreationDate -Descending |
    Select-Object ProcessId, CreationDate, CommandLine |
    Format-List
