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
}
finally {
    Pop-Location
}

Write-Host "parity checks passed"
