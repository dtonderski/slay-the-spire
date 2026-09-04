# Simulator

The simulator area owns deterministic game mechanics and all infrastructure
needed to validate them. It must not depend on anything under [`../rl/`](../rl/README.md).
RL code may consume the simulator's public APIs; the dependency is one-way:
`rl -> simulator`.

## Layout

- `crates/sts_core/`: authoritative mechanics, state, actions, snapshots, and RNG.
- `crates/sts_env/`: fair observations and decision-local policy environment.
- `crates/sts_verify/`: strict CommunicationMod trace replay.
- `bindings/py_sts/`: PyO3 binding crate.
- `python/`: the `sts_sim` Python package and API example.
- `verification/`: committed fixtures and the ignored permanent trace corpus.
- `tools/communication/`: bridge, immutable trace collector, and JavaScript tests.
- `mods/`: collection support mods.
- `docs/`: simulator research, verification, and Python API documentation.

## Rust validation

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo run -p sts_verify --bin sts_verify -- simulator/verification/corpus/permanent_traces
```

The verifier accepts a schema-6/7 trace file or a directory of those traces.
The committed `corpus/manual/milestone1.jsonl` file is a unit-test fixture, not
a strict verifier trace.

## Python binding

```bash
uv sync --project simulator/python --reinstall-package sts-sim
cd simulator/python
uv run maturin develop --uv
uv run ty check
uv run ruff check sts_sim examples
uv run python examples/showcase.py
```

See [`docs/python_api.md`](docs/python_api.md).

## Collection tools

```bash
node simulator/tools/communication/trace_client.test.js
node simulator/tools/communication/random_fidelity_collector.test.js
node simulator/tools/communication/run_random_fidelity_campaign.test.js
node simulator/tools/communication/trace_ui/server.test.js
```

Real-game bridge setup is documented in
[`tools/communication/README.md`](tools/communication/README.md). Captured traces
are immutable; verification and corpus promotion remain separate operations.
