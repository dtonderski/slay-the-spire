# Slay the Spire Ironclad Simulator

This project aims to build a headless Rust simulator for Slay the Spire, starting with the Ironclad.

The long-term goal is to support reinforcement learning agents, planning, deterministic replay, and eventually full-run evaluation. The short-term goal is much smaller: build a faithful simulator through tiny tested tasks, not a giant unverified rewrite.

## Repository Layout

The repository root is the Rust workspace. Simulator mechanics, search,
verification, bindings, and live tooling have separate owners:

- `crates/sts_core/`: deterministic simulator mechanics and run state.
- `crates/sts_search/`: deterministic planning over authoritative run state.
- `crates/sts_verify/`: strict seed-plus-actions trace replay.
- `bindings/py_sts/`: PyO3 bindings for the Python package.
- `apps/sts_live/`: live trace backend, CLI, HTTP API, and Vite UI.
- `python/`: one Python project containing `sts_sim` and `sts_sim.rl`.
- `verification/`: compact fixtures and the gitignored permanent parity corpus.
- `docs/`: project and component documentation.
- `tools/communication/`: required CommunicationMod client bridge and diagnostics.
- `mods/`: small project-specific game mods.

## Current Scope

The active repository is centered on the Rust simulator, live collector, and
verification corpus.

## Verification Philosophy

The simulator should be deterministic from seed plus action trace. It should be verified with:

- unit tests for local mechanics
- golden tests for small transitions
- snapshot round trips
- deterministic replay
- CommunicationMod-style comparisons against the real game when parity is claimed

[CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod) and [spirecomm](https://github.com/ForgottenArbiter/spirecomm) are important reference tools for real-game state export and control.

## RL Use

Future RL agents should use the simulator through a clean environment API:

- reset
- legal action generation
- step
- snapshot/restore
- symbolic observations
- typed Python bindings

Simulator mechanics must stay separate from RL feature extraction and reward shaping.

### Beam-cloning training

The optional RL package can generate deterministic legal simulator roots, label
each public decision with the shared Rust beam teacher, train with exact
batch-boundary resume, and evaluate a fixed development shard. From `python/`
after `uv sync --extra rl && uv run maturin develop --uv`:

```bash
uv run sts-combat-data roots --output /tmp/sts-roots \
  --seed-prefix BEAMCLONE --count 1000
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
uv run sts-combat-evaluate \
  --dataset /tmp/sts-development/dataset-manifest.json \
  --checkpoint /tmp/sts-checkpoint.pt --split development \
  --output /tmp/sts-development-report.json
```

`sts-combat-evaluate` is static imitation accuracy on already-labeled public
decisions. `sts-combat-rollout` is a separate diagnostic: it independently
restores identical split-root snapshot bytes and compares a seeded SHA-256
random policy, a greedy network that scores only `FairCombatObservation` plus
current public action descriptors, and the native privileged replanning beam.
Errors and truncations stay in the win-rate denominator. The rollout report is
not a promotion or replacement claim.

```bash
uv run sts-combat-rollout \
  --roots /tmp/sts-roots/root-manifest.json \
  --checkpoint /tmp/sts-checkpoint.pt --split development \
  --seed 0 --output /tmp/sts-gameplay-report.json
```

Roots advance only through accepted public legal actions. Dataset generation
copies the canonical named root manifest to `provenance/root-manifest.json` and
accounts for every requested root as either a membership or a typed exclusion.
Training records contain fair observations and public choices, never hidden
state, RNG, native handles, or snapshots. Audited splits require explicit
`roots --materialize-audited-splits` and `--allow-audited-split`; these are
fail-closed workflow defaults, not filesystem security. Derived roots, datasets,
and checkpoints are disposable and should be regenerated after source/layout
changes rather than migrated by weakening their source digests.

## Local Validation

Run Rust and verifier checks from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo run -p sts_verify --bin sts_verify -- corpus manual/milestone1.jsonl
```

Build and test the Python project from `python/`:

```bash
cd python
uv sync --extra rl --reinstall-package sts-sim
uv run maturin develop --uv
uv run pytest -q
uv run ty check
```

Full CommunicationMod payloads are external. Set `STS_PERMANENT_CORPUS_DIR` or
place the reviewed cohort under `verification/corpus/permanent_traces/`; traces
are expected output and are never used to hydrate simulator state.

## Project Documents

- `docs/research.md`: prior-art and source notes
- `docs/design.md`: architecture and risk analysis
- `PROJECT_OVERVIEW.md`: high-level RL roadmap, phase gates, evaluation protocol, and state-visibility boundaries
- `docs/verification.md`: parity and testing strategy
- `docs/live_trace_ui_design.md`: live trace collection UI design
- `AGENTS.md`: repository-wide rules for coding agents
