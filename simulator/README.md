# simulator

Rust workspace for the Slay the Spire simulator and verification crates. Core game logic lives in `crates/sts_core`; sim-to-real trace tooling lives in `crates/sts_verify`.

Plain `cargo test` runs the default Rust workspace crates: `sts_core`,
`sts_verify`, and `sts_live`. The `py_sts` PyO3 binding crate remains a
workspace member, but is tested explicitly because Windows needs the uv-managed
Python runtime directory on `PATH` when running the generated Rust test binary.

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

To run the Rust unit tests inside the PyO3 binding crate:

```powershell
$env:PYO3_PYTHON = (Resolve-Path .venv\Scripts\python.exe).Path
$pythonBase = & $env:PYO3_PYTHON -c "import sys; print(sys.base_prefix)"
$env:PATH = "$pythonBase;$env:PATH"
cargo test -p py_sts --lib
```

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
