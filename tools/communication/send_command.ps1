param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Command
)

$ErrorActionPreference = "Stop"

$sessionDir = Join-Path $PSScriptRoot "session"
$commandPath = Join-Path $sessionDir "next_command.txt"

if (-not (Test-Path $sessionDir)) {
    throw "Session directory not found: $sessionDir. Is the bridge client running?"
}

Set-Content -Path $commandPath -Value $Command -NoNewline
Write-Output "Sent command: $Command"
