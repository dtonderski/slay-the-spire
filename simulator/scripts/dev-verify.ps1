param(
    [string[]]$Test = @(),
    [string]$TestPackage = "sts_core",
    [switch]$NoLib,
    [string[]]$Trace = @(),
    [switch]$SkipFmt,
    [switch]$SkipClippy,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Body
    )

    Write-Host ""
    Write-Host "==> $Name"
    & $Body
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

$simRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) {
    $cargo = "cargo"
}
$uv = "uv"

Push-Location $simRoot
try {
    if (-not $SkipFmt) {
        Invoke-Step "cargo fmt" {
            & $cargo fmt
        }
    }

    foreach ($filter in $Test) {
        Invoke-Step "cargo test -p $TestPackage $filter" {
            $args = @("test", "-p", $TestPackage, $filter)
            if (-not $NoLib) {
                $args += "--lib"
            }
            & $cargo @args
        }
    }

    if (-not $SkipClippy) {
        Invoke-Step "uv run cargo clippy" {
            & $uv run cargo clippy
        }
    }

    if (-not $SkipBuild) {
        Invoke-Step "uv run maturin develop --release" {
            & $uv run maturin develop --release
        }
    }

    foreach ($tracePath in $Trace) {
        $resolvedTrace = Resolve-Path $tracePath
        Invoke-Step "strict replay $resolvedTrace" {
            $python = @'
from pathlib import Path
import json
import sys
from sts.self_play import strict_replay_real_trace_to_env

path = Path(sys.argv[1])
result = strict_replay_real_trace_to_env(trace=path)
print('TRACE', path.name)
print('verified', result.verified)
print('stop_reason', result.stop_reason)
print('steps', result.steps)
print('final_state_id', result.final_state_id)
print('final_phase', result.final_phase)
blocker = result.blocker or {}
slim = {key: blocker.get(key) for key in ['trace_step', 'command', 'category', 'reason']}
slim['diffs'] = blocker.get('diffs')
print(json.dumps(slim, indent=2)[:24000])
if blocker:
    print('sim', json.dumps(blocker.get('simulator_summary'), indent=2)[:12000])
raise SystemExit(0 if result.verified else 1)
'@
            & $uv run python -c $python $resolvedTrace
        }
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "dev verification passed"
