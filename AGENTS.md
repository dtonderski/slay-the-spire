# Agent Rules

These rules apply across Rust, Python, bindings, collection tools, mods, and
verification. `PROJECT_OVERVIEW.md` defines the project objective and fair-state
boundary.

## Never

- Never hydrate, synchronize, repair, re-anchor, or otherwise mutate simulator
  state from observed trace/game state. Replay advances only from initial state,
  accepted actions, explicit environmental inputs, and implemented rules.
- Never edit, truncate, or regenerate a captured trace to make replay pass.
- Never add seed-, trace-, or corpus-specific behavior to implementation code.
- Never hide gameplay-affecting differences or claim parity without a real-game
  trace.
- Never apply effects in the wrong order and then restore selected fields to
  match an observation. Whole-state rollback for rejected transitions is fine;
  post-hoc correction of accepted gameplay is not.

## Testing

Traces are the primary gameplay regression. Unit tests are appropriate for
infrastructure, deterministic invariants, and source-backed rules that no trace
can pin.

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo run -p sts_verify --bin sts_verify -- <trace.jsonl|trace-directory>
```

`sts_verify` accepts exactly one file or directory and caps directory replay at
24 workers. It reports divergence, incomplete traces, and invalid input as
failures.

Python and PyO3 commands go through `uv`; there is no required system
`python`/`pip`.

## Determinism

- No untracked global RNG.
- No RNG during legal-action generation, serialization, hashing, observation
  extraction, or display.
- Every RNG draw names its stream and call site.
- Snapshot/restore preserves replay behavior exactly.

## Corpus and collection

`simulator/verification/corpus/permanent_traces/` holds the reviewed schema-6
corpus. The verifier accepts schemas 6 and 7; schema 7 adds stronger
command-settlement fences for new captures. Never rewrite old payloads.

`simulator/tools/communication/random_fidelity_collector.js` only collects
immutable traces. Verification and promotion are separate manual operations.

Real-game control uses the CommunicationMod bridge documented in
`simulator/tools/communication/README.md`, not handcrafted socket commands.

## Working practice

- Keep searches under `tmp/decompiled-sts/` targeted to one package path.
- If a missing dependency materially improves correctness, stop and report it
  rather than building an inferior workaround.
- Read `simulator/docs/research.md` before changing RNG, action queues, save
  loading, or map/reward/shop generation.
- Update `docs/project_history.md` only for major assumptions, rejected
  approaches, or settled experiments. Git and commit messages hold routine
  implementation history; do not create per-fix design documents.

## Cursor Cloud

`.cursor/install.sh` installs Rust, `uv`, and Python build dependencies;
`.cursor/start.sh` creates/downloads the active corpus when `HF_TOKEN` is
available. Supported cloud scope is `sts_verify` and `py_sts`; real-game bridge
work requires the local game.
