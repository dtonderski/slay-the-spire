# Agent Rules for Future Coding Sessions

These rules are for Codex or any other coding agent working on this project.

## Core Rules

1. Implement only one task from `TASKS.md` at a time.
2. Add tests before or with implementation.
3. Run `cargo fmt`, `cargo clippy`, and `cargo test` from `simulator/` before declaring a simulator code task complete.
4. Update `STATUS.md` in the same change as the task.
5. Never continue to a new task with failing tests.
6. Never add unrelated mechanics.
7. Write a short design note before risky mechanics.
8. Preserve deterministic behavior.
9. Keep simulator logic separate from RL feature extraction.
10. Do not implement simulator code during design-only tasks.
11. Read `RESEARCH.md` before implementing RNG, action queue, save loading, map generation, reward generation, shop generation, or real-game verification tools.

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

- Unit-test local rules.
- Golden-test complete transitions.
- Add regression tests for every bug fix.
- Add serialization round-trip tests when state shape changes.
- Add replay tests when transition behavior changes.
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

Every completed task updates `STATUS.md` with:

- completed task title
- test commands run
- current milestone
- next task
- known risks or limitations
