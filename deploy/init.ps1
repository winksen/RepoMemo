# Makes the .env of a container install on Windows (V0.4.23): the host name and strong secrets, from .env.example. It
# refuses to overwrite a .env that exists, because replacing the secrets of a running install would sign everyone out
# and make the stored provider keys unreadable.
#
#   deploy\init.ps1                                  a .env for localhost, with the edge (TLS) profile
#   deploy\init.ps1 -HostName memo.example.com       for a public name (Let's Encrypt gets the certificate)
#   deploy\init.ps1 -NoEdge                          no TLS piece: a proxy of your own is in front
#   deploy\init.ps1 -Output C:\path\.env             another file than .\.env
param(
    [string]$HostName = "localhost",
    [switch]$NoEdge,
    [string]$Output = ".env"
)
$ErrorActionPreference = "Stop"

function Fail([string]$Message) {
    Write-Error "init: $Message"
    exit 1
}

$example = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "..\.env.example"
if (-not (Test-Path $example)) { Fail "cannot find .env.example next to this script ($example)" }
if (Test-Path $Output) { Fail "$Output exists already; not touching it. Remove it first if you mean to start again (that signs everyone out)" }
if ($HostName -notmatch '^[A-Za-z0-9.-]+$') { Fail "the host name ($HostName) may hold only letters, digits, dots and hyphens" }

function New-Secret {
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    -join ($bytes | ForEach-Object { $_.ToString("x2") })
}

$jwt = New-Secret
$key = New-Secret
$profiles = if ($NoEdge) { "" } else { "edge" }

$lines = Get-Content -Path $example | ForEach-Object {
    if ($_ -match '^REPOMEMO_HOST=') { "REPOMEMO_HOST=$HostName" }
    elseif ($_ -match '^REPOMEMO_JWT_SECRET=') { "REPOMEMO_JWT_SECRET=$jwt" }
    elseif ($_ -match '^REPOMEMO_SECRET_KEY=') { "REPOMEMO_SECRET_KEY=$key" }
    elseif ($_ -match '^COMPOSE_PROFILES=') { "COMPOSE_PROFILES=$profiles" }
    else { $_ }
}
# UTF-8 without a byte-order mark, which Docker Compose reads as part of the first name.
[System.IO.File]::WriteAllLines((Join-Path (Get-Location) $Output), $lines, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "init: wrote $Output for $HostName (edge: $(-not $NoEdge))."
Write-Host "Keep a copy of REPOMEMO_SECRET_KEY somewhere safe: without it the AI provider keys and repository tokens stored in the"
Write-Host "database must be entered again. Then:"
Write-Host "  docker compose pull; docker compose up -d --wait"
Write-Host "and open https://$HostName (or http://127.0.0.1:8080 without the edge) to finish the first-run setup."
