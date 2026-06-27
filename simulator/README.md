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
