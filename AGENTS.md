# Agent Rules

These rules apply to any coding agent working in this repository.
`simulator/AGENTS.md` adds rules specific to `simulator/`.

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

- Cargo runs from `simulator/`; there is no root `Cargo.toml`. Gate work on
  `cargo fmt`, `cargo clippy`, `cargo test`, and don't start new work with tests
  failing.
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
- `simulator/verification/corpus/permanent_traces/` is ~15 GB and gitignored.
  Never commit it. Details in `simulator/docs/verification.md`.
- Keep searches targeted. `tmp/decompiled-sts/`, when extracted, is a huge
  uncommitted decompiled-source corpus: search one package path such as
  `com/megacrit/cardcrawl/monsters/`, never the whole tree.

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
