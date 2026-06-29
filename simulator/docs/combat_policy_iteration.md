# Combat Policy Iteration

This is the short orientation page for simulator-backed combat policy work. Use
it to find the data, understand the evaluation boundaries, and decide where to
record new results. The detailed history lives in
`simulator/docs/combat_autopilot_iterations.md`.

## Purpose

Combat policy iteration is for automating combat while replaying higher-level
Slay the Spire decisions. The current policies are non-ML and intentionally
omniscient: they can inspect exact simulator state, legal actions, and cloned
futures. That is acceptable for combat automation and trace generation. It is
not a fair RL/player-facing policy.

## Data Locations

- Real captured CommunicationMod traces:
  `verification/corpus/communication_mod/`
- Real trace provenance and cleanliness notes:
  `verification/corpus/communication_mod/trace_notes.md`
- Generated trace-guided replay and eval artifacts:
  `simulator/target/trace-guided/`
- Search-lab reports:
  `simulator/target/search-lab/`
- Policy-iteration runs:
  `simulator/target/policy-iteration/<run-id>/`
- Current generated-data map:
  `simulator/docs/data_manifest.md`

`simulator/target/` is generated output. It is useful locally, but the durable
record should be the command, metrics, lessons, and canonical run path in docs.

## Held-Out MANUAL01

Canonical current held-out replay:

- `simulator/target/trace-guided/manual01-strict-rerun-replay.jsonl`

Canonical strict report:

- `simulator/target/trace-guided/manual01-strict-rerun-report.json`

The held-out replay is used for final combat-policy comparisons only. Do not
tune candidate parameters directly on MANUAL01. Train/dev iteration should use
simulator-generated corpora first, then MANUAL01 should be run as the scoreboard.

Clean held-out replay requirements:

- Strict report has `verified = true`.
- Strict report has `anchor_count = 0`.
- Strict report has `restoration_count = 0`.
- Strict report has `blocker = null`.
- `verify_self_play_trace(...)` over the replay returns `ok = true`,
  `repair_anchor_count = 0`, and `restoration_count = 0`.

Current MANUAL01 comparison surfaces:

- `combat_start`: 21 roots.
- `all_decision_states`: 333 roots.
- Potion mode: `trace_used`, meaning a policy may use potion types only on roots
  where the real trace used them.

`simulator/target/trace-guided/manual01-replayed.jsonl` is older diagnostic
history. It helped expose replay-anchor and baseline bugs, but it is not the
current held-out artifact.

## Run Layout

`iterate-combat-policy` writes one run directory:

```text
simulator/target/policy-iteration/<run-id>/
```

Expected files:

- `iteration-report.json`: inspect this first.
- `train-sim/index.json` and `train-sim/traces/*.jsonl`.
- `dev-sim/index.json` and `dev-sim/traces/*.jsonl`.
- `train_combat_start.json` and `train_all.json`.
- `dev_combat_start.json` and `dev_all.json`.
- `heldout_manual01_combat_start.json`.
- `heldout_manual01_all.json`.
- Matching `*-failures.json` files.

`iteration-report.json` records the candidate list, promoted dev candidate,
strict held-out replay verification, compact train/dev/held-out reports, worst
regressions, best improvements, and blockers.

## Current Canonical Run

Current run:

- `simulator/target/policy-iteration/manual01-heldout-v5/`

Summary:

- Train corpus: 4 simulator traces, 4 verified.
- Dev corpus: 2 simulator traces, 2 verified.
- Blockers: none.
- Promoted from simulator dev combat-start:
  `rust_beam_terminal_w32_d40`.
- Held-out strict replay verification:
  `ok = true`, `repair_anchor_count = 0`, `restoration_count = 0`,
  `steps = 531`.

Held-out MANUAL01 v5 scoreboard:

| Scope | Roots | Best ranked candidate | Mean HP loss | Delta vs trace | Wins/Losses/Nonterminal |
| --- | ---: | --- | ---: | ---: | --- |
| `combat_start` | 21 | `rust_greedy_tactical_d40` | 16.524 | +7.286 | 20/0/1 |
| `all_decision_states` | 333 | `rust_beam_terminal_w32_d40` | 10.249 | +3.952 | 328/3/2 |

Ranking note: ranking is not pure mean HP loss. Terminal outcome is part of the
ordering, so a lower-HP-loss candidate can rank lower if it loses or fails to
terminate more often.

## Current Experimental Candidate

The current best exploratory candidate is:

- `rust_terminal_hp_commit_safe_selector_w32_w64_d40`

Current held-out MANUAL01 probe reports live under:

- `simulator/target/policy-iteration/superhuman-v1/`

Summary:

| Scope | Roots | Mean HP loss | Delta vs trace | Wins/Losses/Nonterminal | Mean sec/decision |
| --- | ---: | ---: | ---: | --- | ---: |
| `combat_start` | 21 | 7.333 | -1.905 | 21/0/0 | 0.174 |
| `all_decision_states` | 333 | 5.348 | -0.949 | 333/0/0 | 0.098 |

Status:

- This clears the current super-human held-out gate: better than the human trace
  by mean HP loss on both held-out scopes, with no losses and no nonterminal
  episodes.
- Always-on width-128 Rust beam can stack-overflow on the step-407 multi-enemy
  root. The safe selector avoids width 128.
- The selector also excludes the `hp_preserving_lethal` width-64 branch after it
  stack-overflowed on step 414.
- The current result depends on two general policy fixes: purpose-aware
  select-screen confirmation and a boss nonterminal survival tie-break.

## Commit Policy

Commit:

- Docs and experiment summaries.
- Small hand-curated real traces or clean prefixes when intentionally promoted.
- `trace_notes.md` updates explaining provenance, cleanliness, and caveats.
- Minimized regression traces only when they become verification fixtures.

Do not commit:

- Bulk generated data under `simulator/target/`.
- `tools/communication/session/`.
- `.tmp/`, `.venv/`, `__pycache__/`, wheels, and build outputs.
- Raw exploratory reports unless they are intentionally promoted and documented.

## Experiment Logging

Use `simulator/docs/combat_autopilot_iterations.md` for durable conclusions.
Record:

- Hypothesis.
- Command run.
- Input trace/replay path.
- Candidate names.
- Train/dev/held-out separation.
- Root scope: `combat_start` or `all_decision_states`.
- Potion mode, especially `trace_used`.
- Metrics: roots, wins/losses/nonterminal, mean/median HP loss, delta vs trace
  where valid, potion uses, runtime per decision, and worst regressions.
- Decision: promoted, rejected, or diagnostic only.
- Links/paths to generated reports, without checking those reports in.

Failed probes are worth recording when they change future behavior. Keep them
short: what failed, why it matters, and what not to repeat.
