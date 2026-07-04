param(
    [string]$HostName = "127.0.0.1",
    [int]$Port = 8799,
    [switch]$NoStop
)

$ErrorActionPreference = "Stop"

function Stop-UiService {
    $matches = Get-CimInstance Win32_Process |
        Where-Object {
            $_.CommandLine -and
            ($_.Name -in @("python.exe", "uv.exe")) -and
            ($_.CommandLine -match "sts\.ui_service")
        }

    foreach ($process in $matches) {
        Write-Host "Stopping UI process $($process.ProcessId): $($process.CommandLine)"
        Stop-Process -Id $process.ProcessId -Force
    }
}

$simRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$uv = "uv"
$url = "http://${HostName}:$Port/"

if (-not $NoStop) {
    Stop-UiService
}

Write-Host "Starting UI service at $url"
$process = Start-Process `
    -FilePath $uv `
    -ArgumentList @("run", "python", "-m", "sts.ui_service") `
    -WorkingDirectory $simRoot `
    -WindowStyle Hidden `
    -PassThru

$deadline = (Get-Date).AddSeconds(15)
do {
    Start-Sleep -Milliseconds 500
    try {
        $response = Invoke-WebRequest -UseBasicParsing $url -TimeoutSec 2
        if ($response.StatusCode -eq 200) {
            Write-Host "UI ready at $url"
            exit 0
        }
    }
    catch {
        if ($process.HasExited) {
            throw "UI service exited early with code $($process.ExitCode)"
        }
    }
} while ((Get-Date) -lt $deadline)

throw "UI service did not become ready at $url"
