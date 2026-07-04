# Agent Rules for Future Coding Sessions

These rules are for Codex or any other coding agent working on this project.

## Core Rules

1. Keep each change scoped to one coherent task at a time.
2. Add tests before or with implementation.
3. Run `cargo fmt`, `cargo clippy`, and `cargo test` from `simulator/` before declaring a simulator code task complete.
4. Document important verification, risks, and follow-up work in the commit message or a permanent project document when the change needs it.
5. Never continue to a new task with failing tests.
6. Never add unrelated mechanics.
7. Write a short design note before risky mechanics.
8. Preserve deterministic behavior.
9. Keep simulator logic separate from RL feature extraction.
10. Do not implement simulator code during design-only tasks.
11. Read `RESEARCH.md` before implementing RNG, action queue, save loading, map generation, reward generation, shop generation, or real-game verification tools.
12. If a missing dependency or tool would materially simplify the task, improve correctness, or avoid a substantially worse workaround, stop and tell the user. Do not quietly build an inferior workaround around a missing crucial dependency.

## Search Scope / Context Hygiene

- Do not run broad repository-wide searches that include `tmp/decompiled-sts/`,
  generated files, build outputs, or vendor/dependency trees.
- Treat `tmp/decompiled-sts/` as a large local reference corpus. When inspecting
  decompiled Slay the Spire sources, search only the relevant package or class
  path, for example `tmp/decompiled-sts/com/megacrit/cardcrawl/monsters/...`.
- Prefer targeted `rg` commands with explicit paths or globs over root-level
  `rg --files` or broad keyword searches.
- If a broad inventory is needed, exclude heavy trees explicitly, for example
  `rg --files -g '!tmp/decompiled-sts/**'`.
- Keep command output budgets small for exploratory searches, then widen only
  after the target files are known.

## Determinism Rules

- No untracked global RNG.
- No RNG during legal action generation, serialization, hashing, observation extraction, or display.
- Every RNG draw must name its stream and call site.
- Snapshot/restore must preserve replay behavior exactly.
- State hashes must be deterministic.

## Scope Control

- If a task requires another mechanic, stop and split the prerequisite into its own task.
- If a mechanic is tempting but not required by the current task, document it in `STATUS.md` or `TASKS.md` instead of coding it.
- Do not add all cards, all relics, all monsters, or all events in bulk.

## Testing Rules

- For simulator fidelity bugs found through real play, prefer adding or
  extending CommunicationMod trace replay coverage over adding narrow unit
  tests. The trace is the primary regression.
- Keep unit tests small and rare for simulator mechanics. Use them when they
  protect infrastructure/parsers, serialization, deterministic invariants, or a
  tiny source-backed rule that a trace cannot isolate cleanly.
- Do not add broad gameplay unit tests that merely encode the agent's current
  interpretation of Slay the Spire. Source-backed code plus trace replay is
  preferred.
- Golden-test complete transitions when a compact transition fixture is clearer
  than a full trace.
- Add regression tests for every bug fix.
- Add serialization round-trip tests when state shape changes.
- Add replay tests when transition behavior changes; for real-game parity work,
  this should usually mean a persistent trace or manifest entry.
- Add property tests for invariants when the affected state can be randomly generated.

## Verification Rules

- Do not claim real-game parity without a real-game trace or an explicitly stated reason.
- Prefer CommunicationMod-style JSON traces for exact comparison.
- Use `sts_lightspeed` as useful prior art and a secondary differential oracle, not as the final authority.
- Treat wiki and community references as starting points, not final proof.
- Mark hidden or unobservable fields explicitly.
- Never make diffs pass by silently ignoring gameplay-affecting state.

## Rust Hygiene

- Prefer simple enums and structs over broad abstraction.
- Avoid macros unless they remove real repetitive risk.
- Avoid dynamic plugin-style content systems until repeated implemented mechanics justify them.
- Keep public APIs small and documented.
- Use `serde` for snapshots when implementation begins.

## RL Boundary

- Core simulator returns symbolic state and legal actions.
- RL wrappers may create tensors, action masks, reward shaping, and batched stepping.
- RL code must not duplicate game mechanics.
- If feature extraction needs derived values, compute them outside authoritative simulator state unless they are part of game state.

## Status Discipline

When completing a task, leave enough durable context for the next session to
understand what changed, what was verified, and what remains risky. Prefer the
commit message for ordinary implementation notes, and use permanent project
documents only for decisions or design context that should outlive a single
change.
