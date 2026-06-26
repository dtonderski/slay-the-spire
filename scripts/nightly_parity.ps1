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

    & $cargo run -q -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-21T09-57-10-380Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    # Expected-failing M29 Sentries cleaned trace: zero unexpected diffs, but it ends
    # on the final reward screen before the post-reward PROCEED boundary.
    & $cargo run -q -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & $cargo run -q -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
finally {
    Pop-Location
}

Write-Host "parity checks passed"
