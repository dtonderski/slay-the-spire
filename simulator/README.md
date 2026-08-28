# Simulator

Rust workspace for the deterministic Slay the Spire simulator, live collector,
combat agent, Python bindings, and strict real-game verifier.

## Layout

- `crates/sts_core/`: simulator mechanics and deterministic run state.
- `crates/sts_verify/`: strict seed-plus-actions trace replay.
- `crates/sts_live/`: CommunicationMod bridge backend, CLI, UI, and combat agent.
- `crates/py_sts/`: optional PyO3 bindings.
- `verification/corpus/`: compact, reviewable verification fixtures.
- `docs/`: simulator design and verification notes.

## Verification

From this directory:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
export STS_PERMANENT_CORPUS_DIR=/path/to/permanent_traces
uv run -- cargo run -p sts_verify -- parity \
  "$STS_PERMANENT_CORPUS_DIR/<trace>.jsonl"
```

The verifier always derives simulator state from the recorded `START` seed and
subsequent actions. Real-game observations are comparison evidence, never
simulator-state hydration.

Full CommunicationMod payloads are external. Point
`STS_PERMANENT_CORPUS_DIR` at that directory and pass a trace path explicitly.
To export the authoritative simulator endpoint from a trace, use:

```bash
export STS_PERMANENT_CORPUS_DIR=/path/to/permanent_traces
cargo run -p sts_verify -- replay --json \
  "$STS_PERMANENT_CORPUS_DIR/<trace>.jsonl" -o replay.json
cargo run -p sts_verify -- replay --json --at-step 3322 \
  "$STS_PERMANENT_CORPUS_DIR/<trace>.jsonl"
```

Replay returns the final snapshot, a state-hash checkpoint for each trace
action, and an optional snapshot at or before `--at-step`. It exits `0` when
the trace reaches its endpoint, `1` for invalid input, and `2` when the
authoritative simulator reaches a replay boundary. Observed game state never
changes the exported snapshot.

## Beam-cloning training

The optional Python RL package can generate deterministic legal simulator roots,
label them with the incumbent production beam planner, train with exact
batch-boundary resume, and evaluate a fixed development shard. From
`simulator/python` after `uv sync --extra rl && uv run maturin develop --uv`:

```bash
uv run sts-combat-data roots \
  --output /tmp/sts-roots --seed-prefix BEAMCLONE --count 1000
uv run sts-combat-data label \
  --roots /tmp/sts-roots/root-manifest.json \
  --output /tmp/sts-train --split train
uv run sts-combat-data label \
  --roots /tmp/sts-roots/root-manifest.json \
  --output /tmp/sts-development --split development
uv run sts-combat-train \
  --dataset /tmp/sts-train/dataset-manifest.json \
  --checkpoint /tmp/sts-checkpoint.pt --steps 1000
uv run sts-combat-train \
  --dataset /tmp/sts-train/dataset-manifest.json \
  --checkpoint /tmp/sts-checkpoint.pt --steps 1000 --resume
uv run sts-combat-evaluate \
  --dataset /tmp/sts-development/dataset-manifest.json \
  --checkpoint /tmp/sts-checkpoint.pt --split development \
  --output /tmp/sts-development-report.json
```

Root generation advances seeded runs only through accepted public legal actions.
Root files and dataset shards are canonical SHA-256 artifacts. Training records
contain fair observations and public action descriptors, never native handles,
internal IDs, RNG state, or snapshots. `sealed_test` and `real_trace_audit`
loaders fail closed unless an audited caller opts in. PUCT and candidate
promotion remain later phases; the commands above implement deterministic beam
imitation only.

## Live CLI

CommunicationMod launches `..\tools\communication\trace_client.js`. With the
game bridge running, use the Linux CLI from WSL:

```bash
cd /mnt/d/dev/slay-the-spire/simulator
export STS_LIVE_BRIDGE_SESSION_DIR=/mnt/d/dev/slay-the-spire/tools/communication/session
cargo run -p sts_live --bin live-trace -- bridges list
cargo run -p sts_live --bin live-trace -- sessions list
```

If CommunicationMod was launched from another worktree, point
`STS_LIVE_BRIDGE_SESSION_DIR` at that worktree's shared session directory.
Keep `STS_LIVE_ALLOW_FILE_COMMANDS` unset: normal play uses guarded TCP control.

Replay a verified CommunicationMod or `sts_live` JSONL trace into the real game
with the same Linux CLI:

```bash
cargo run -p sts_live --bin live-trace -- replay /path/to/source.jsonl --dry-run
cargo run -p sts_live --bin live-trace -- replay /path/to/source.jsonl \
  --bridge communication-mod --reset-bridge
```

Replay validates the source against the simulator before touching the game. It
starts the recorded character, ascension, and seed, requires any captured
profile input to match the live profile, then matches each recorded command to
one current enabled legal action. It stops before an unavailable command or as
soon as live fidelity is not `ok`. Without `--reset-bridge`, an active run is
never abandoned. Use `--max-actions N` to replay only a verified prefix after
`START`; `--dry-run` performs no bridge operations. Normal live replay does not
accept `START_VERIFY` traces or traces with explicit boss-unlock inputs that the
live bridge cannot assert.
