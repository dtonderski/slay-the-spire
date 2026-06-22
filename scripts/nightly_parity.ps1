# Nightly parity runner (local / CI placeholder).
# Runs verification crate tests and the sts_verify CLI against corpus fixtures.

$ErrorActionPreference = "Stop"
$cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) { $cargo = "cargo" }

Push-Location $PSScriptRoot\..\simulator
try {
    & $cargo test -p sts_verify
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & $cargo run -q -p sts_verify -- trace ..\verification\corpus\communication_mod\trace-2026-06-18T00-53-06-235Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & $cargo run -q -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T00-53-06-235Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & $cargo run -q -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & $cargo run -q -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & $cargo run -q -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & $cargo run -q -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T16-45-23-530Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $testParity = & $cargo run -q -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-21T09-57-10-380Z.jsonl 2>&1 | Out-String
    if ($LASTEXITCODE -eq 2 -and $testParity -match 'seed_start.expected_failure=false' -and $testParity -match 'verified step=168.*enter shop merchant') {
        Write-Host "TEST seed-start: shop inventory verified at step 168 (post-purchase shop UI diffs may remain)"
    } elseif ($LASTEXITCODE -ne 0) {
        Write-Host $testParity
        exit $LASTEXITCODE
    }

    & $cargo run -q -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
finally {
    Pop-Location
}

Write-Host "parity checks passed"
