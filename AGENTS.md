# Agent Rules

These rules apply across Rust, Python, bindings, collection tools, mods, and
verification. Read `PROJECT_OVERVIEW.md` before implementation work and preserve
its fair-state boundary.

`rl` may depend on `simulator`; `simulator` must not depend on `rl`.

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

## Subagent model policy

- NEVER use Astra (including `gpt-6-astra`, aliases, and variants) for subagents,
  regardless of role, harness, fallback, or thinking level. It consumes the
  user's limited quota. Astra is reserved for the parent session only.
- Explicitly select and verify a non-Astra model for every child launch. Never
  inherit the parent model implicitly. Do not resume an Astra-backed child.
- If a safe non-Astra model cannot be verified, stop and ask; do not fall back
  to Astra. This applies to workers, scouts, reviewers, delegates, and oracles.

## Testing

Traces are the primary gameplay regression evidence. Unit tests may supplement
them and cover infrastructure, deterministic invariants, and source-backed rules,
but do not establish real-game parity.

For Rust changes, run from the repository root; for gameplay or replay changes,
also replay the reviewed corpus:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo run -p sts_verify --bin sts_verify -- simulator/verification/corpus/permanent_traces
```

`sts_verify` accepts exactly one file or directory and caps directory replay at
24 workers. It reports divergence, incomplete traces, and invalid input as
failures.

Use `simulator/README.md` for Python and collection-tool validation commands when
changing those areas. Python and PyO3 commands go through `uv`; there is no
required system `python`/`pip`.

Report which checks ran and any failures or checks that could not run. If the
reviewed corpus is unavailable, report that limitation; fixtures are not a
substitute for corpus replay.

## Determinism

- No untracked global RNG.
- No RNG during legal-action generation, serialization, hashing, observation
  extraction, or display.
- Gameplay RNG draws use explicit named streams and preserve call-site attribution
  in the RNG tracing infrastructure. Non-seeded gameplay draws require typed,
  call-time external inputs as documented in `simulator/docs/research.md`.
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
- If a required dependency or authoritative source is unavailable and proceeding
  would require guessing gameplay behavior, report the blocker rather than
  inventing a substitute. Continue independent work that is not blocked.
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
