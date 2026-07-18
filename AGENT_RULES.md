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
- If a mechanic is tempting but not required by the current task, document it in a commit message or permanent project document instead of coding it.
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
- When iterating on simulator fidelity from a collected trace, use `sts-verify`
  or the repo's trace replay verifier against the saved CommunicationMod trace.
  Do not restart the live backend/UI as the inner debugging loop. Restart
  `live-trace` only after the verifier-driven fix is ready to validate in the
  UI or to serve the updated implementation.
- Use `sts_lightspeed` as useful prior art and a secondary differential oracle, not as the final authority.
- Treat wiki and community references as starting points, not final proof.
- Mark hidden or unobservable fields explicitly.
- Never make diffs pass by silently ignoring gameplay-affecting state.
- Never anchor, re-anchor, synchronize, restore, repair, hydrate, or otherwise
  mutate simulator state from observed trace/game state during replay or
  verification. A trace observation is expected output only. The simulator must
  advance solely from the initial seed/state plus accepted actions and
  implemented game rules. If simulated state diverges from observed state, stop
  at the first divergence and fix the simulator bug.

## Live Backend / UI Rules

- The Vite UI at `http://127.0.0.1:5173/` proxies backend requests to
  `http://127.0.0.1:8800`, as configured in
  `simulator/crates/sts_live/ui/vite.config.ts`. When restarting the live trace
  backend for the UI, run `live-trace serve --addr 127.0.0.1:8800`; do not use
  the binary default `8799` unless intentionally bypassing the UI proxy.
- After restarting the backend, verify both `http://127.0.0.1:8800/health` and
  `http://127.0.0.1:5173/health`. If direct health is connected but the UI says
  "backend disconnected", check the Vite proxy port before changing backend
  code.

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

## Project History Curation

- Maintain `PROJECT_HISTORY.md` as a compact explanation of why the project took
  its current shape. The default is no update: edit it only when work changes a
  durable project-level belief or decision, reveals a rejected approach worth
  remembering, or establishes an experiment conclusion that changes future work.
- Do not copy routine implementation details, file lists, test commands, commit
  summaries, transient status, or raw debugging notes into the history. Git and
  task-specific status documents already preserve those.
- Make at most one bounded history edit per coherent task, normally no more than
  150 words. Prefer revising an existing section over adding a heading. Most
  tasks should make no edit.
- Curate instead of endlessly appending: merge repetition, remove superseded
  wording, and keep the main file below roughly 3,000 words. Preserve an old
  belief only when the reason it changed is part of the useful story. Near the
  size limit, merge or remove at least as much as you add.
- Distinguish fact from reconstruction. Link durable evidence when useful, and
  label uncertain retrospective claims rather than presenting them as settled.
- Do not use Current Thesis or Open Strategic Questions as status/backlog
  sections, and do not record speculative future plans unless they are adopted.
- Use this test: if forgetting the information would not make a future agent
  repeat a failed direction or misunderstand an important architectural choice,
  omit it.
