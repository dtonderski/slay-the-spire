# Agent Rules

These rules apply to any coding agent working in this repository, including
Rust code under `crates/`, bindings under `bindings/`, and live tooling under
`apps/`. `simulator/AGENTS.md` adds rules only for the Python, verification,
documentation, and other content that temporarily remains under `simulator/`.

`PROJECT_OVERVIEW.md` has the project purpose, phase roadmap, and the
fair/hidden/omniscient state boundary.

## Never

- Never anchor, re-anchor, synchronize, restore, repair, hydrate, or otherwise
  mutate simulator state from observed trace/game state during replay or
  verification. A trace observation is expected output only. The simulator must
  advance solely from the initial seed/state plus accepted actions and
  implemented game rules. If simulated state diverges from observed state, stop
  at the first divergence and fix the simulator bug.
- Never edit, truncate, or regenerate a captured trace to make replay pass.
  Captured payloads are unreproducible real-game evidence; a corrupted one
  reports green forever. Repair simulator behavior and replay the unchanged
  payload.
- Never add seed-specific behavior to implementation code: no `if seed == ...`,
  no trace identity tables, no hardcoded RNG counters, no per-trace allowlists.
  Fixed seeds are fine in tests and corpus metadata.
- Never make a diff pass by excluding gameplay-affecting state, and never claim
  real-game parity without a real-game trace.
- Never implement a successful gameplay transition by applying effects in the
  wrong order and then copying/restoring selected fields from a pre-transition
  snapshot to match the observed result. Model action ordering, queued effects,
  cancellation, and conditional execution directly. Whole-state transactional
  rollback on a rejected/error transition is still allowed; post-hoc gameplay
  correction on an accepted transition is not.

## Testing

Traces are the primary regression mechanism. A gameplay unit test encodes your
interpretation of the game; a trace encodes the game. Keep unit tests for
simulator mechanics small and rare, and reach for trace coverage first.

Unit tests earn their place for infrastructure (parsers, importers, CLI, HTTP,
FFI, serialization round-trips), for deterministic invariants, and for a
source-backed rule no trace can pin.

For a bug fix, the regression is the payload: a trace that stays green. Write a
unit test instead only when no trace can pin the rule.

The inner loop is the Rust verifier, not the UI:
`uv run -- cargo run -p sts_verify --bin sts_verify -- parity <trace.jsonl>`
(also `status`, `replay`, `minimize`, `diff`, `rng-trace`, `corpus`).

## Determinism

- No untracked global RNG.
- No RNG during legal action generation, serialization, hashing, observation
  extraction, or display.
- Every RNG draw names its stream and its call site.
- Snapshot/restore preserves replay behavior exactly, and state hashes are
  deterministic.

## Environment

- Cargo runs from the repository root. Gate work on `cargo fmt`,
  `cargo clippy`, `cargo test`, and don't start new work with tests failing.
  Use `cargo check -p sts_core --lib` and `cargo check -p sts_verify --lib` for
  fast feedback.
- Python and PyO3 go through `uv`, e.g.
  `uv run --python 3.12 cargo test --workspace`. There is no system
  `python`/`pip`, and its absence is not a blocker.
- The live backend must bind `127.0.0.1:8800`
  (`live-trace serve --addr 127.0.0.1:8800`), *not* the binary default `8799`,
  because the Vite UI on `:5173` proxies to `8800`. If direct `/health` works but
  the UI says disconnected, check the proxy port before touching backend code.
- Real-game control from WSL or a sandbox goes through the Linux `live-trace` CLI
  with host-network permission and `STS_LIVE_BRIDGE_SESSION_DIR` set to the
  active CommunicationMod `tools/communication/session`. Not PowerShell, not
  handcrafted TCP. A sandboxed port check cannot prove the Windows game is
  stopped; use `live-trace bridges list`.
- `simulator/verification/corpus/permanent_traces/` is the gitignored active
  authoritative corpus. The private `dtonderski/sts-permanent-traces` Hugging
  Face dataset mirrors the current reviewed cohort. The 602 pre-collection.2
  payloads remain a local legacy archive and must not be copied back into the
  active gate. Details are in `simulator/docs/verification.md`.
- Keep searches targeted. `tmp/decompiled-sts/`, when extracted, is a huge
  uncommitted decompiled-source corpus: search one package path such as
  `com/megacrit/cardcrawl/monsters/`, never the whole tree.

### Cursor Cloud

- Cloud agents download the current reviewed permanent corpus from the private
  `dtonderski/sts-permanent-traces` Hugging Face dataset at boot. `HF_TOKEN`
  must be a read token supplied through Cursor secrets, never committed.
- Cloud agents may read and replay the downloaded traces but must never edit
  them or upload corpus changes. Corpus replacement is an explicit audited
  local operation.

## Trace Collection

`tools/communication/random_fidelity_collector.js` only collects: it plays
random actions against the real game and writes one immutable trace per run.
It does not verify, minimize, or promote. Verification is a separate manual step
through `sts_verify`. Keep it that way.

## Dependencies

If a missing dependency or tool would materially simplify the task, improve
correctness, or avoid a substantially worse workaround, stop and tell the user.
Do not quietly build an inferior workaround around a missing crucial dependency.

## Docs

`docs/project_history.md` explains why the project has its current shape. Update
it when a change alters a large assumption, rejects an approach, or settles an
experiment — not for routine work. Keep it curated and under roughly 3,000
words, revising existing sections rather than appending. Everything else belongs
in the commit message; do not open a new document for ordinary supporting
evidence.

Read `docs/research.md` before touching RNG, action queue, save loading, or
map/reward/shop generation.

## Cursor Cloud specific instructions

Environment setup lives in `.cursor/`: `install.sh` (via `environment.json`
`install`) installs newest stable Rust plus `uv`/`libpython3.12-dev` and builds
`py_sts` on Cursor's default image — no custom Dockerfile, so it works for
just-in-time agents too. `start.sh` creates the active corpus directory but does
not restore the legacy archive. Edit those files, not a dashboard snapshot.
Supported Cloud scope is `sts_verify` and `py_sts`; the `sts_live` live UI needs
a real game/CommunicationMod bridge that cannot run here. Exercise the verifier
without active traces using the committed fixture:
`cargo run -p sts_verify --bin sts_verify -- corpus manual/milestone1.jsonl`
plus the `milestone*` tests.
- `cargo test --workspace` has one parallelism-sensitive test,
  `rng::tests::rng_trace_capture_restores_disabled_fast_path_after_panic` (it
  asserts on the process-global `RNG_TRACE_ACTIVE` counter). Run it in isolation
  or with `-- --test-threads=1`; it is a harness race, not a simulator bug.
- Two `py_sts` failures are pre-existing in a clean checkout, not setup breakage:
  `pytest test_content_catalogues_are_complete_python_enums` (its hardcoded card
  count is stale vs. the catalogue) and a Ruff `F401` in
  `python/notebooks/fair_combat_playground.ipynb`. `ty check` and the rest of
  `pytest` pass.
