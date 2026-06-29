# Simulator Data Manifest

This file records where local generated data lives and which artifacts are
canonical for current combat-policy work. Most paths below are under
`simulator/target/`; they are generated local artifacts and should usually not be
checked in. Check in the code, commands, metrics, and lessons. Regenerate the
data when needed.

## Rules of Thumb

- Treat `simulator/target/` as generated output.
- Keep a checked-in note for any run that informs policy decisions.
- Do not tune candidates directly on held-out real traces.
- If a verifier, strict replay, or exact-action invariant fails, stop the
experiment and fix that first.
- Prefer a new run directory over mutating an old report.

## Held-Out Real Trace

Canonical held-out replay:

- `simulator/target/trace-guided/manual01-strict-rerun-replay.jsonl`

Strict verifier report:

- `simulator/target/trace-guided/manual01-strict-rerun-report.json`

This is the clean MANUAL01 real-trace replay used as held-out evaluation. It is
not training data and should not drive candidate parameter choices directly.

Cleanliness requirements:

- `verified = true` in the strict report.
- `anchor_count = 0`.
- `restoration_count = 0`.
- `blocker = null`.
- `verify_self_play_trace(...)` over the replay reports `ok = true`,
  `repair_anchor_count = 0`, and `restoration_count = 0`.

Current replay facts:

- Steps: 531.
- Final phase: `idle`.
- Combat-start roots: 21.
- All in-combat decision roots: 333.

Older diagnostic replay:

- `simulator/target/trace-guided/manual01-replayed.jsonl`

Do not use `manual01-replayed.jsonl` as the current held-out scoreboard. It was
useful while fixing replay, anchor, and baseline semantics, but the strict rerun
above is the current clean artifact.

## Policy Iteration Runs

Policy iteration outputs live under:

- `simulator/target/policy-iteration/<run-id>/`

Each run directory should contain:

- `iteration-report.json`: compact top-level summary, candidates, promoted
  candidate, blocker list, strict held-out replay verification, and links to
  detailed reports.
- `train-sim/`: simulator-generated train traces and `index.json`.
- `dev-sim/`: simulator-generated dev traces and `index.json`.
- `train_combat_start.json`.
- `train_all.json`.
- `dev_combat_start.json`.
- `dev_all.json`.
- `heldout_manual01_combat_start.json`.
- `heldout_manual01_all.json`.
- `*-failures.json` files with concrete failure fixtures.

Canonical current run:

- `simulator/target/policy-iteration/manual01-heldout-v5/`

Current v5 summary:

- Train corpus: 4 simulator traces, 4 verified.
- Dev corpus: 2 simulator traces, 2 verified.
- Candidates:
  - `rust_terminal_win_hp_bounded_w32_d40`
  - `rust_beam_terminal_w32_d40`
  - `rust_beam_terminal_w16_d40`
  - `rust_greedy_tactical_d40`
- Promoted from simulator dev combat-start:
  `rust_beam_terminal_w32_d40`.
- Blockers: none.
- Held-out replay verification:
  `ok = true`, `repair_anchor_count = 0`, `restoration_count = 0`,
  `steps = 531`.

Held-out MANUAL01 v5 scoreboard:

| Scope | Roots | Best ranked candidate | Mean HP loss | Delta vs trace | Wins/Losses/Nonterminal |
| --- | ---: | --- | ---: | ---: | --- |
| `combat_start` | 21 | `rust_greedy_tactical_d40` | 16.524 | +7.286 | 20/0/1 |
| `all_decision_states` | 333 | `rust_beam_terminal_w32_d40` | 10.249 | +3.952 | 328/3/2 |

Ranking note: ranking is not pure mean HP loss. Terminal outcomes matter, so a
candidate with worse mean HP loss can rank above another candidate if it avoids
losses.

## Current Experimental Scoreboard

Current exploratory run directory:

- `simulator/target/policy-iteration/superhuman-v1/`

This directory contains policy-quality probes after the clean v5 harness was in
place. It is not a replacement for the v5 iteration report because these probes
are hand-launched experiments, not one complete `iterate-combat-policy` run.

Important artifacts:

- `dev_all_safe_selectfix2.json`
- `heldout_manual01_combat_start_selectfix2.json`
- `heldout_manual01_all_selectfix2.json`

Current best clean held-out probe:

| Scope | Candidate | Roots | Mean HP loss | Delta vs trace | Wins/Losses/Nonterminal | Mean sec/decision |
| --- | --- | ---: | ---: | ---: | --- | ---: |
| `combat_start` | `rust_terminal_hp_commit_safe_selector_w32_w64_d40` | 21 | 7.333 | -1.905 | 21/0/0 | 0.174 |
| `all_decision_states` | `rust_terminal_hp_commit_safe_selector_w32_w64_d40` | 333 | 5.348 | -0.949 | 333/0/0 | 0.098 |

Interpretation:

- The safe selector beats the human trace by mean HP loss on both held-out
  scopes and has no losses or nonterminal episodes on either held-out root
  scope.
- Steps 241 and 242 were fixed by purpose-aware select-screen confirmation:
  Burning Pact-style single-card exhaust screens now confirm after one valid
  selection instead of selecting extra cards until confirm becomes illegal.
- Step 439 was fixed by a boss nonterminal survival selector: when no candidate
  is a terminal win against a single large monster, prefer non-lost candidates
  that preserve player HP instead of blindly minimizing monster HP.
- The safe selector avoids known crashing branches: unconditional width-128 is
  not used, and the `hp_preserving_lethal` width-64 branch is excluded after it
  stack-overflowed on step 414.

## Diagnostic Policy Runs

Older policy-iteration directories are useful as debugging history, not as
current scoreboards:

- `manual01-heldout-v1`: stalled after `train_combat_start`; default candidate
  set included slow wide/rescue experiments.
- `manual01-heldout-v2`: exposed an exact-action bug after a Duplication Potion
  state. `exact_legal_actions()` could return `PlayCard(card_id=16)` while
  `step()` rejected the same action with `UnknownCard`.
- `manual01-heldout-v3`: confirmed the legality fix but showed that broad
  validation inside every legal-action call was too slow.
- `manual01-heldout-v4`: showed Python `tactical_greedy_d40` was too slow for
  default full all-state iteration.
- `manual01-heldout-v5`: first clean current iteration report after the
  exact-action fix and all-Rust default candidate set.

## Experiment Log Template

Add an entry to `combat_autopilot_iterations.md` when a run changes policy
direction:

- Heading: `### N. Short Iteration Name`
- Hypothesis: what should improve and why.
- Command: the exact command or command shape.
- Artifacts: generated `target/...` paths.
- Results: roots, candidate, mean HP loss, delta vs trace,
  wins/losses/nonterminal, and mean seconds per decision.
- Decision: promote, reject, or diagnostic only.
- Lessons: what to repeat or avoid.

## Held-Out Cleanliness Rules

- `trace_used` is the canonical MANUAL01 potion mode. Global potion allowlists
  and unrestricted potion probes are diagnostic only.
- Root scopes are not interchangeable. `combat_start` measures full-combat
  automation; `all_decision_states` stress-tests mid-combat recovery and can
  overweight repeated states from one fight.
- Mean delta vs trace is valid only for roots with strict comparable human
  combat baselines. Report missing/invalid baseline counts when using older
  diagnostic traces.
- Any exact-action invariant failure, strict replay failure, anchor/restoration
  in strict mode, or blocked trace extraction invalidates policy conclusions
  until fixed.

## Cleanup Guidance

Before deleting generated data, preserve the relevant summary in docs. It is
usually safe to delete diagnostic `target/policy-iteration/manual01-heldout-v1`
through `v4` after the lessons above are no longer needed locally. Keep or
regenerate `v5` while it is the canonical comparison point.
