# simulator

Rust workspace for the Slay the Spire simulator and verification crates. Core game logic lives in `crates/sts_core`; sim-to-real trace tooling lives in `crates/sts_verify`.

## Python Setup

Use `uv` from this directory:

```powershell
uv sync
uv run maturin develop --release
uv run python -m unittest discover -s python\tests -v
```

The `maturin develop` step installs the local PyO3 extension module into the
`uv` environment. After that, Python tools can be run without setting
`PYTHONPATH` manually.

## Daily Fidelity Loop

Use the helper scripts from this directory for the common edit/test/replay/UI
cycle:

```powershell
.\scripts\dev-verify.ps1 -Test gremlin_nob_ -Trace ..\verification\corpus\communication_mod\<trace>.jsonl
.\scripts\restart-ui.ps1
```

`dev-verify.ps1` runs `cargo fmt`, focused Rust tests, `uv run cargo clippy`,
`uv run maturin develop --release`, and optional strict replay checks. The
script accepts trace paths from the command line; it does not bake in trace
names, seeds, or replay-specific behavior.

`restart-ui.ps1` stops only Python/uv processes whose command line contains
`sts.ui_service`, starts `uv run python -m sts.ui_service`, and waits for the
local UI health check at `http://127.0.0.1:8799/`.
