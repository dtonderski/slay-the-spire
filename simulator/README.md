# Simulator

Rust workspace for the deterministic Slay the Spire simulator, live collector,
combat agent, Python bindings, and strict real-game verifier.

## Layout

- `crates/sts_core/`: simulator mechanics and deterministic run state.
- `crates/sts_verify/`: strict seed-plus-actions trace replay.
- `crates/sts_live/`: CommunicationMod bridge backend, CLI, UI, and combat agent.
- `crates/py_sts/`: optional PyO3 bindings.
- `verification/corpus/`: captured traces and the permanent regression corpus.
- `docs/`: simulator design and status notes.

## Verification

From this directory:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run -- cargo run -p sts_verify -- parity verification\corpus\communication_mod\<trace>.jsonl
```

The verifier always derives simulator state from the recorded `START` seed and
subsequent actions. Real-game observations are comparison evidence, never
simulator-state hydration.

## Live CLI

CommunicationMod launches `..\tools\communication\trace_client.js`. With the
game bridge running, inspect the CLI with:

```powershell
cargo run -p sts_live --bin live-trace -- bridges list
cargo run -p sts_live --bin live-trace -- sessions list
```
