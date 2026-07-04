param(
    [switch]$Full
)

$ErrorActionPreference = "Stop"

$sessionDir = Join-Path $PSScriptRoot "session"
$summaryPath = Join-Path $sessionDir "summary.json"
$statePath = Join-Path $sessionDir "current_state.json"
$statusPath = Join-Path $sessionDir "status.json"

foreach ($path in @($summaryPath, $statusPath)) {
    if (-not (Test-Path $path)) {
        throw "Session file not found: $path. Is the bridge client running?"
    }
}

Get-Content $statusPath -Raw | Write-Output
Write-Output "---"
Get-Content $summaryPath -Raw | Write-Output

if ($Full -and (Test-Path $statePath)) {
    Write-Output "---"
    Get-Content $statePath -Raw | Write-Output
}
