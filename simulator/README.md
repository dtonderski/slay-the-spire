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

## Checked vs normal execution

Ordinary simulation does not run full `RunState` / `CombatState` / map
invariant scans on constructors, legal-action queries, accepted transitions, or
internal mechanics. Those `validate()` methods remain explicitly callable for
tests, snapshot restore, and verifier/import audit. Normal paths still enforce
action legality, stale-revision rejection, targets/costs/phase restrictions,
required environmental inputs, supported-surface/fairness guards, and malformed
load/schema safety. Illegal actions stay atomically rejected; the simulator does
not repair or hydrate state.

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

## Random combat robustness probes

These probes construct explicit synthetic loadouts using the normal encounter
spawn/combat-start pipeline, then sample legal actions with a separate seeded
driver RNG. They do not run training or establish real-game parity.

```bash
cargo build -p sts_core --example combat_fuzz --release
uv run --no-project python simulator/tools/combat_fuzz_campaign.py \
  --binary target/release/examples/combat_fuzz --start 0 --count 1000 \
  --out tmp/combat-fuzz/campaign-0
uv run --no-project python -m unittest discover -s simulator/tools \
  -p combat_fuzz_campaign_test.py
```

Each combat runs in a separate subprocess with a 10-second timeout. Both that
timeout and the 2,000-action bound produce **candidates**, not confirmed hangs.
Campaign directories must be new. They retain an executable hash/copy, revision,
implementation diff, per-case results, and failure artifacts. Failed cases and
interrupted subprocesses retain append-only live journals recording setup,
initial state, accepted actions, and the last attempted operation. Successful
case journals are removed; their outcomes remain in the campaign result log.

To replay a complete failure artifact with a matching build:

```bash
target/release/examples/combat_fuzz replay path/to/seed-304.json
```

The initial coverage includes all four acts, ascensions 0/2/10/17/18/19/20,
random modeled non-status cards, and a curated combat-relic pool. Loadouts are
synthetic, not claims that every combination is reachable in an ordinary run.
Potions and broader relic/counter coverage can be added in later campaigns.

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
