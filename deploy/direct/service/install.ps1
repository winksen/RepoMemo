# Installs the unpacked archive as a Windows service (V0.4.33). Run it in an administrator PowerShell, from anywhere:
#
#   .\service\install.ps1 -GenerateSecrets -Start
#
# What it does, and only this:
#   - copies the release to C:\Program Files\RepoMemo\repomemo-<version> and points the junction ...\RepoMemo\current at it,
#     so a new release is a new junction and the old one stays for a way back (V0.4.36);
#   - makes C:\ProgramData\RepoMemo (where workspace-data\ is written) with an access list: SYSTEM and Administrators full
#     control, the service account (Local Service) change;
#   - makes C:\ProgramData\RepoMemo\repomemo.env from the example IF IT IS NOT THERE; -GenerateSecrets fills in the two
#     secrets; an existing file is never touched;
#   - registers the service RepoMemo (starts when Windows does, runs as Local Service, restarts after a failure) through
#     `repomemo-server service install`; -Start starts it.
# It can be run again: every step checks first, and a run with nothing to change changes nothing.
param(
    [string]$Prefix = (Join-Path $env:ProgramFiles "RepoMemo"),
    [string]$DataDir = (Join-Path $env:ProgramData "RepoMemo"),
    [switch]$GenerateSecrets,
    [switch]$Start
)
$ErrorActionPreference = "Stop"

function Say([string]$Message) { Write-Host "install: $Message" }
function Fail([string]$Message) { Write-Error "install: $Message"; exit 1 }

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not (New-Object Security.Principal.WindowsPrincipal($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Fail "run this in an administrator PowerShell"
}
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
foreach ($required in "bin\repomemo-server.exe", "VERSION", "SHA256SUMS", "config\repomemo.env.example") {
    if (-not (Test-Path (Join-Path $root $required))) { Fail "$root is not an unpacked RepoMemo archive ($required is missing)" }
}
$version = (Get-Content (Join-Path $root "VERSION") -Raw).Trim()
if ($version -notmatch '^[A-Za-z0-9._-]+$') { Fail "the VERSION file holds something that is not a version" }

# The release, and the junction to it.
$release = Join-Path $Prefix "repomemo-$version"
$current = Join-Path $Prefix "current"
New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
$sums = Join-Path $release "SHA256SUMS"
if ((Test-Path $sums) -and ((Get-FileHash $sums).Hash -eq (Get-FileHash (Join-Path $root "SHA256SUMS")).Hash)) {
    Say "release $version is already in $release"
} else {
    if (Test-Path $release) { Fail "$release exists and differs from this archive; move it away first (it is never overwritten)" }
    $staging = Join-Path $Prefix ".repomemo-$version.new"
    if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
    Copy-Item -Recurse -Path $root -Destination $staging
    Move-Item -Path $staging -Destination $release
    Say "copied the release to $release"
}
$item = Get-Item $current -ErrorAction SilentlyContinue
if ($item -and $item.LinkType -eq "Junction" -and ($item.Target -contains $release -or $item.Target -eq $release)) {
    # already pointing at it
} else {
    if ($item -and $item.LinkType -ne "Junction") { Fail "$current exists and is not a junction; move it away first" }
    # A running service holds the old release's files open only by path, so the junction can move under it; the restart picks the new one.
    if ($item) { cmd /c rmdir "$current" | Out-Null }
    New-Item -ItemType Junction -Path $current -Target $release | Out-Null
    Say "$current now points at $release"
}

# The data folder and its access list.
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
& icacls $DataDir /inheritance:r /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" "*S-1-5-19:(OI)(CI)M" | Out-Null
if ($LASTEXITCODE -ne 0) { Fail "could not set the access list of $DataDir" }

# The settings: made once, never touched again.
$envFile = Join-Path $DataDir "repomemo.env"
function New-Secret {
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    -join ($bytes | ForEach-Object { $_.ToString("x2") })
}
if (Test-Path $envFile) {
    Say "$envFile exists; leaving it as it is"
} else {
    $webDir = Join-Path $current "web"
    $lines = Get-Content (Join-Path $root "config\repomemo.env.example") | ForEach-Object {
        if ($_ -match '^REPOMEMO_WEB_DIR=') { "REPOMEMO_WEB_DIR=$webDir" }
        elseif ($GenerateSecrets -and $_ -match '^REPOMEMO_JWT_SECRET=') { "REPOMEMO_JWT_SECRET=$(New-Secret)" }
        elseif ($GenerateSecrets -and $_ -match '^REPOMEMO_SECRET_KEY=') { "REPOMEMO_SECRET_KEY=$(New-Secret)" }
        else { $_ }
    }
    [System.IO.File]::WriteAllLines($envFile, $lines, (New-Object System.Text.UTF8Encoding($false)))
    & icacls $envFile /inheritance:r /grant:r "*S-1-5-18:F" "*S-1-5-32-544:F" "*S-1-5-19:R" | Out-Null
    if ($GenerateSecrets) { Say "made $envFile with new secrets: keep a copy of REPOMEMO_SECRET_KEY somewhere safe" }
    else { Say "made $envFile from the example: fill in REPOMEMO_JWT_SECRET and REPOMEMO_SECRET_KEY before starting" }
}

# The service. Its program is the junction's path, so a new release does not need it registered again.
$exe = Join-Path $current "bin\repomemo-server.exe"
if (Get-Service -Name RepoMemo -ErrorAction SilentlyContinue) {
    Say "the service RepoMemo is already registered"
} else {
    & $exe service install --workdir $DataDir --env-file $envFile
    if ($LASTEXITCODE -ne 0) { Fail "could not register the service" }
}

if ($Start) {
    if (Select-String -Path $envFile -Pattern '^REPOMEMO_(JWT_SECRET|SECRET_KEY)=\s*$' -Quiet) { Fail "not starting: $envFile still has an empty secret" }
    if ((Get-Service RepoMemo).Status -eq "Running") { Restart-Service RepoMemo } else { Start-Service RepoMemo }
    (Get-Service RepoMemo).WaitForStatus("Running", [TimeSpan]::FromSeconds(60))
    Say "started: Get-Service RepoMemo; the logs are in $DataDir\workspace-data\main\logs"
} else {
    Say "done. Start it with: Start-Service RepoMemo"
}
