# Removes the Windows service of RepoMemo (V0.4.33). Run it in an administrator PowerShell.
#
#   .\service\uninstall.ps1                  stops and unregisters the service; the program files and the data stay
#   .\service\uninstall.ps1 -RemoveProgram   also deletes the program files (C:\Program Files\RepoMemo)
#
# The data (C:\ProgramData\RepoMemo: the database, the stored files, the backups, the settings and the key file) is never
# deleted by this script. Remove that folder yourself when you are sure you do not need it.
param(
    [string]$Prefix = (Join-Path $env:ProgramFiles "RepoMemo"),
    [switch]$RemoveProgram
)
$ErrorActionPreference = "Stop"
$exe = Join-Path $Prefix "current\bin\repomemo-server.exe"
if (Get-Service -Name RepoMemo -ErrorAction SilentlyContinue) {
    if (Test-Path $exe) { & $exe service uninstall } else { & sc.exe stop RepoMemo | Out-Null; & sc.exe delete RepoMemo | Out-Null }
    Write-Host "uninstall: the service RepoMemo is removed"
} else {
    Write-Host "uninstall: the service RepoMemo is not installed"
}
if ($RemoveProgram -and (Test-Path $Prefix)) {
    $current = Join-Path $Prefix "current"
    if (Test-Path $current) { cmd /c rmdir "$current" | Out-Null }
    Remove-Item -Recurse -Force $Prefix
    Write-Host "uninstall: removed $Prefix (the data in C:\ProgramData\RepoMemo was not touched)"
}
