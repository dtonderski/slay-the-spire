# Simulator Data, Python, and Documentation

The Rust workspace now lives at the repository root. This directory temporarily
retains the Python project, verification data, and component documentation until
the final physical move.

## Layout

- `../crates/sts_core/`: simulator mechanics and deterministic run state.
- `../crates/sts_search/`: deterministic planning over authoritative run state.
- `../crates/sts_verify/`: strict seed-plus-actions trace replay.
- `../apps/sts_live/`: CommunicationMod bridge backend, CLI, UI, and combat agent.
- `../bindings/py_sts/`: PyO3 bindings.
- `python/`: Python package and tests.
- `verification/corpus/`: compact, reviewable verification fixtures.
- `docs/`: simulator design and verification notes.

## Verification

From the repository root:

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
label them with a beam policy that intentionally replans at every public decision,
train with exact batch-boundary resume, and evaluate a fixed development shard.
The teacher reuses the shared beam search core but does not claim the live
automation warm-suffix execution policy. From
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
  --checkpoint /tmp/sts-checkpoint.pt --steps 1000 \
  --minimum-roots 225 --minimum-lineages 100
# If that run is interrupted, repeat the same immutable schedule with --resume.
# A completed 1000-step checkpoint resumed toward 1000 is already complete.
uv run sts-combat-evaluate \
  --dataset /tmp/sts-development/dataset-manifest.json \
  --checkpoint /tmp/sts-checkpoint.pt --split development \
  --output /tmp/sts-development-report.json
```

Root generation advances seeded runs only through accepted public legal actions.
Root and dataset generation require an empty output directory, so stale sealed
roots or shards cannot survive an ordinary rerun. Root files and dataset shards
are canonical SHA-256 artifacts. Each dataset carries the canonical named root
manifest at `provenance/root-manifest.json`; loading re-resolves every successful
membership and typed per-root labeling exclusion against it, and requires every
source root in the requested split to be accounted for exactly once. A native
teacher failure excludes only that root with a stable reason and public diagnostic;
labeling continues with later roots. If every root fails, generation publishes no
dataset. Training size and lineage gates count successful memberships only.
Training records contain fair observations and public action descriptors, never native handles,
internal IDs, RNG state, or snapshots. Default root generation withholds audited
split snapshots and membership. Materializing them requires
`roots --materialize-audited-splits`; labeling or loading them separately requires
`--allow-audited-split`, and all paths are split-isolated. These are fail-closed
tool defaults and audit metadata, not cryptographic authorization against the
local filesystem owner. Training config V1 refuses fewer than 225 roots or 100
distinct canonical lineages by default; `--minimum-roots` and
`--minimum-lineages` exist for explicit versioned tests and smoke runs, not for
lowering the production gate. Checkpoint resume and evaluation strictly match
Python, NumPy, Torch, platform, deterministic CPU/thread policy, source, and
`pyproject.toml`/`uv.lock` identity. PUCT and candidate promotion remain later
phases; the commands above implement deterministic public-decision replanning
beam imitation only.

## Live CLI

CommunicationMod launches `..\tools\communication\trace_client.js`. With the
game bridge running, use the Linux CLI from WSL:

```bash
cd /mnt/d/dev/slay-the-spire
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
