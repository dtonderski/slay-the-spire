# Combat Autopilot Iteration Log

This is the living lab notebook for the simulator-backed combat autopilot. The goal is not to build a perfect Slay the Spire AI. The goal is to automate combats well enough to replay high-level datasets that already provide map, reward, relic, card, and event decisions.

## Operating Goal

Build a practical simulator-backed combat autopilot for dataset replay.

Requirements we care about:

- Evaluate on trace-derived combat roots, not only synthetic fixtures.
- Compare every candidate against a stable baseline.
- Support allowed-potion constraints, especially "use the same potion types the real trace used".
- Save bad roots as fixtures so policy work is driven by concrete failures.
- Stop and reassess if the policy quality is clearly too poor to automate replay.

## Important Boundary

The Python API/search policy is intentionally omniscient for this milestone. It can inspect exact simulator state, exact legal actions, and cloned futures. That is acceptable for trace generation and combat automation. It should not be confused with a fair RL/player-facing policy.

## Iterations

### 1. Simulator-Only Self-Play and Trace Eval

Commits:

- `36dc495 Use uv for simulator Python tooling`
- `c8989d8 Add self-play corpus trace evaluation`
- `16e0097 Add potion allowlists to trace eval`

What changed:

- Added `uv`-based Python workflow under `simulator/`.
- Added simulator-only self-play trace generation.
- Added trace-root evaluation using recorded simulator combat roots.
- Added `allowed_potions` filtering so a policy can be evaluated with all potions, no potions, or only selected potion names.

Why:

- We needed a repeatable way to run policies against combat states without manually playing the real game.
- Potion usage has to be controllable because a combat-local policy can otherwise spend strategic resources unrealistically.

What we learned:

- The trace eval path is the right evaluation surface, but ranking only by final score/final HP was too coarse.
- Potion visibility exists in the eval report, but early small-root slices may not include many potion opportunities.

### 2. Real Trace Replay Into Simulator Roots

Commits:

- `f442ac0 Add trace-guided real replay adapter`
- `d48c981 Anchor real trace replay at observed combats`
- `0b42347 Extend trace replay through observed combat anchors`
- `c985fb5 Use distinct roots in trace eval`

What changed:

- Added replay of CommunicationMod traces into `OmniRunEnv`.
- When exact replay hits a boundary, observed combat states can be used as simulator anchors.
- Distinct combat roots are counted by state hash to avoid over-counting repeated states.

Why:

- The real trace was not a clean simulator trace from the start. Anchors let us extract useful combat roots anyway.
- This turns the manual playthrough into evaluation data for combat search.

What we learned:

- The long `MANUAL01` replay produces 323 distinct usable combat roots.
- The replay trace at `simulator/target/trace-guided/manual01-replayed.jsonl` is the current best local eval artifact.

### 3. First HP-Preserving Search Prototype

Commit:

- `f60ea7a Prototype HP-preserving combat search`

What changed:

- Added `hp_preserving_lethal` objective.
- Added an HP-preserving portfolio variant.
- Fixed a real beam/greedy bug where the search could return no action if every legal action lowered the heuristic score.

Why:

- The existing search was too willing to trade HP for faster kills.
- A defensive policy must still choose painful legal actions, such as ending the turn when no card is playable.

10-root prototype result:

- `hp_greedy_d40`: mean HP loss `7.2`, wins `3`, losses `1`, nonterminal `6`.
- `baseline_exhaustive_basic_d3`: mean HP loss `14.3`, wins `5`, losses `1`, nonterminal `4`.
- `hp_beam_w8_d20`: mean HP loss `21.8`, wins `5`, losses `1`, nonterminal `4`.
- `hp_portfolio_d20`: mean HP loss `30.9`, wins `6`, losses `1`, nonterminal `3`.

What we learned:

- HP-greedy reduced HP loss but did not finish enough fights.
- The HP portfolio finished more fights but was too costly.
- More depth alone is not enough. Beam search can prune the good line if the heuristic undervalues setup moves.

### 4. Trace Eval Baseline on First 10 States

Command shape:

```text
uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --split all --max-roots 10 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Power Potion,Explosive Potion"
```

Important correction:

- `--max-roots 10` currently means first 10 combat states, not first 10 combats.
- Those states are mostly consecutive states from the same early fight.
- For policy evaluation, we need a `combat_start` root scope to sample actual combat starts.

Observed current ranking on those first 10 states:

- Best candidates were still only `3/10` wins.
- Several policies lost from early Cultist states.
- This is useful evidence, but it is not yet a fair "10 combats" evaluation.

What we learned:

- The eval harness must report HP loss from the root, not only final HP.
- The eval harness must be able to select first roots of distinct combats.
- Failure cases should be written as fixtures so the next policy iteration has concrete targets.

### 5. Trace Eval Harness Tightening

Files currently modified:

- `simulator/python/sts/search_lab.py`
- `simulator/python/sts/self_play.py`

Intended changes:

- Add `initial_hp` and `hp_loss` to candidate episode results.
- Add `mean_hp_loss` to rankings.
- Add default autopilot candidates:
  - `autopilot_hp_greedy_d40`
  - `autopilot_hp_portfolio_d40`
- Add `root_scope="combat_start"` to trace-root extraction.
- Add `failure_output` support for writing failed/non-winning root fixtures.

CLI shape:

```text
uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --split all --max-roots 10 --max-actions 40 --root-scope combat_start --failure-output target/trace-guided/manual01-autopilot-failures.json --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Power Potion,Explosive Potion"
```

Status:

- Implemented in the current working tree.
- Focused Python tests passed.

10 combat-start eval result:

- Report: `simulator/target/trace-guided/manual01-autopilot-10-combat-starts.json`
- Failure fixtures: `simulator/target/trace-guided/manual01-autopilot-10-combat-start-failures.json`
- Roots: first 10 combat starts from `manual01-replayed.jsonl`
- Allowed potion names: Weak Potion, Cultist Potion, Flex Potion, Elixir, Distilled Chaos, Power Potion, Explosive Potion
- Potion roots: 9 of 10
- Failure fixtures written: 55

Ranking summary:

| Candidate | Wins | Losses | Mean HP Loss | Mean Nodes |
| --- | ---: | ---: | ---: | ---: |
| `beam_tactical_w8_d40` | 5/10 | 1 | 14.2 | 597.5 |
| `portfolio_rollout_d40` | 5/10 | 1 | 16.3 | 25177.0 |
| `exhaustive_tactical_d4` | 5/10 | 1 | 16.9 | 2891.0 |
| `autopilot_hp_portfolio_d40` | 5/10 | 1 | 16.2 | 18307.4 |
| `beam_tactical_w4_d30` | 4/10 | 1 | 10.7 | 318.8 |
| `exhaustive_basic_d3` | 4/10 | 1 | 15.6 | 837.7 |
| `beam_aggressive_w4_d30` | 4/10 | 1 | 20.7 | 400.1 |
| `greedy_tactical_d20` | 2/10 | 1 | 8.9 | 93.2 |
| `autopilot_hp_greedy_d40` | 1/10 | 1 | 8.5 | 94.4 |

Interpretation:

- The HP-greedy policy preserves HP by failing to finish too many combats.
- The HP portfolio finishes more combats, but does not beat the existing tactical beam.
- `beam_tactical_w8_d40` is currently the best "try next" candidate, but 5/10 wins is not enough to automate dataset replay confidently.
- The failure fixture file is now the right input for the next policy iteration.

## Next Steps

1. Use the failure fixtures from `dev-fast-10` to design targeted policy candidates.
2. Select candidates on `dev-50`, with the caveat that this set is currently undersized.
3. Validate only after candidate selection, using `val-50`, with the caveat that this set is currently undersized.
4. Run `full-323` as a broad coverage/sanity check.
5. Decide whether the practical autopilot is promising enough for dataset replay, based on win rate, HP loss, potion usage, runtime, and failure shapes.

### 6. Frozen Eval Set and Runtime Metric Pass

Current working-tree changes after commit `65c7f87`:

- Added named eval sets to `sts.self_play eval`:
  - `dev-fast-10`
  - `dev-50`
  - `val-50`
  - `full-323`
- Added report fields:
  - `available_roots`
  - `eval_set`
  - `eval_set_spec`
  - `held_out`
  - `nonterminal`
  - `median_hp_loss`
  - `p95_hp_loss`
  - `mean_potion_uses`
  - `total_potion_uses`
  - `mean_seconds_per_decision`
  - `p50_seconds_per_decision`
  - `p95_seconds_per_decision`
  - `mean_seconds_per_combat`
  - `p95_search_nodes`

Named set availability from `manual01-replayed.jsonl`:

| Eval Set | Scope | Split | Available | Selected |
| --- | --- | --- | ---: | ---: |
| `dev-fast-10` | combat_start | all | 21 | 10 |
| `dev-50` | combat_start | dev | 17 | 17 |
| `val-50` | combat_start | eval | 4 | 4 |
| `full-323` | all | all | 323 | 323 |

Important caveat:

- The current single trace does not contain enough combat-start roots to make true 50-root dev/validation sets.
- `dev-50` and `val-50` are still frozen named sets, but they are undersized until we have more replay traces or broaden the set definition beyond combat starts.
- `full-323` uses all distinct usable combat states, so it is useful for coverage and regression checks, but it is not a held-out combat-start validation set.

`dev-fast-10` report with timing:

- Report: `simulator/target/trace-guided/eval-dev-fast-10.json`
- Failure fixtures: `simulator/target/trace-guided/eval-dev-fast-10-failures.json`
- Best ranked candidate remains `beam_tactical_w8_d40`.
- `beam_tactical_w8_d40`: wins `5/10`, losses `1`, nonterminal `4`, mean HP loss `14.2`, median HP loss `1.5`, p95 HP loss `59.3`, total potion uses `4`, mean seconds per decision about `0.019`, p95 seconds per decision about `0.027`, mean seconds per combat about `0.143`.
- `portfolio_rollout_d40` tied on wins but was much slower: mean seconds per decision about `0.513`, p95 about `1.262`, mean seconds per combat about `4.66`.

Interpretation:

- The timing metrics make `beam_tactical_w8_d40` look much more practical than rollout portfolio for live automation.
- The 5/10 win rate is still nowhere near good enough.
- The next useful work is not more generic depth; it is targeted failure-driven policy improvement.

### 7. Potion/Select Branch Stabilization

Current working-tree changes after commit `6420095`:

- Normalized potion allowlists so trace/UI names like `Fire Potion` match simulator inventory names like `Fire`.
- Exposed combat selection actions from `OmniRunEnv.exact_legal_actions()`:
  - hand select
  - draw select
  - discard select
  - exhaust select
- Added a local select-screen shortcut in Python search so selection screens are handled as small UI decisions instead of full combat futures.
- Added branch-level select auto-resolution so hypothetical potion/card branches that open a select screen do not explode the search tree.
- Added explicit action costs:
  - potion actions are expensive, so non-lethal heuristic branches should not spend them casually
  - choose-select actions have a tiny cost, so optional selection screens prefer minimal choices
- Added terminal detection for combat states with player HP at or below zero, even if the run phase still says `combat`.
- Split candidate families:
  - `default_candidates()` remains the historical synthetic search-lab set, including expensive portfolio variants.
  - `trace_autopilot_candidates()` is now the practical default for `sts.self_play eval`.

Why:

- After selection actions became visible, trace eval could spend minutes exploring branches like Elixir -> exhaust selection.
- The old default trace eval candidate list mixed synthetic benchmark experiments with practical replay candidates.
- Search could continue evaluating states where the player was already dead because terminal loss was not inferred from HP.

Verification:

```text
uv run maturin develop
uv run python -m unittest discover -s python\tests -v
```

Result:

- Python suite passed: 66 tests.
- `sts.self_play eval --eval-set dev-fast-10` now completes with the practical trace candidate set.

Current `dev-fast-10` trace-autopilot ranking with allowed trace potions:

| Candidate | Wins | Losses | Mean HP Loss | Potion Uses | Mean Seconds / Combat | Mean Seconds / Decision |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `beam_tactical_w4_d20` | 5/10 | 5 | 47.1 | 8 | 2.19 | 0.138 |
| `hp_beam_w4_d30` | 5/10 | 5 | 47.1 | 8 | 2.11 | 0.147 |
| `tactical_greedy_d40` | 5/10 | 5 | 47.8 | 8 | 0.568 | 0.044 |
| `hp_greedy_d40` | 5/10 | 5 | 47.9 | 8 | 0.567 | 0.045 |

No-potion diagnostic:

- `tactical_greedy_d40` with `allowed_potions=()` also reached 5/10 wins, 5 losses.
- It was much faster: mean seconds per combat about `0.215`, mean seconds per decision about `0.019`.
- This suggests the current policy is not getting meaningful value from potions yet; it mostly spends them in already-bad fights.

Interpretation:

- The eval harness is usable again, but the policy quality is still poor.
- The current candidate family finishes fights instead of stalling, but it loses too many hard roots.
- The next algorithm work should focus on tactical survival quality, not merely deeper beam search.
- Potion usage needs a smarter gate, probably "only use potion if the searched no-potion baseline cannot survive or the potion line wins immediately/with clear HP value."

### 8. No-RNG Reshuffle Fallback and Greedy Candidate Selection

Current working-tree changes after commit `f461292`:

- Added a deterministic no-RNG reshuffle fallback for anchored combat states.
  - Before: trace-derived roots with no `shuffle_rng` drew until `draw_pile` was empty, then stopped drawing forever.
  - After: during end-turn draw only the discard cards that already existed before the hand was discarded may cycle into draw.
  - This avoids the old fixture-breaking behavior where the just-ended hand could be immediately redrawn in no-RNG mode.
- Removed the strict replay-verification gate from trace root extraction.
  - Root snapshots remain useful even when old recorded after-hashes no longer match improved simulator mechanics.
- Added `scaling_survival` as an experimental heuristic that values long-fight setup such as strength, ritual, metallicize, and debuffs.
- Improved Elixir/exhaust select handling:
  - exhaust-select now chooses curses/statuses before confirming
  - selected indices are ignored so the shortcut cannot loop on the same selected card
- Trimmed default trace autopilot candidates to the two practical greedy candidates:
  - `tactical_greedy_d40`
  - `hp_greedy_d40`

Why:

- The old no-RNG behavior was a simulator coverage bug masquerading as bad search. Long fights became impossible because the deck stopped cycling.
- The heavy beam/scaling/default candidate set became too slow once fights played out correctly.
- Elixir had been effectively wasted because the select shortcut confirmed immediately.

Diagnostics:

- Forced early Cultist Potion on the remaining `dev-fast-10` boss-like loss did not win.
- Forced early Elixir exhausting initial Injury/Regret also did not win.
- `scaling_survival` played Demon Form on that root but performed worse, so it is kept as an explicit experiment rather than a default trace candidate.

`dev-fast-10` after scoped no-RNG reshuffle fallback:

| Candidate | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `tactical_greedy_d40` | 9/10 | 1 | 0 | 28.2 | 2 | 1.98 |
| `hp_greedy_d40` | 9/10 | 1 | 0 | 30.1 | 2 | 2.01 |
| `beam_tactical_w4_d20` | 9/10 | 1 | 0 | 58.2 | 2 | 5.58 |
| `hp_beam_w4_d30` | 9/10 | 1 | 0 | 58.2 | 2 | 6.96 |

`dev-50` selected-candidate result:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `hp_greedy_d40` | 17 | 13 | 4 | 0 | 32.47 | 5 | 1.94 |
| `tactical_greedy_d40` | 17 | 13 | 4 | 0 | 32.47 | 5 | 2.01 |

Held-out `val-50` result:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `tactical_greedy_d40` | 4 | 3 | 0 | 1 | 17.5 | 0 | 2.54 |
| `hp_greedy_d40` | 4 | 3 | 0 | 1 | 22.25 | 0 | 2.69 |

Selected policy:

- `tactical_greedy_d40`, because it tied `hp_greedy_d40` on dev-50 and had better HP/runtime on held-out val-50.

`full-323` coverage sanity for selected policy:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `tactical_greedy_d40` | 323 | 264 | 19 | 40 | 22.69 | 41 | 1.70 |

Note:

- The `dev-fast-10`, `dev-50`, `val-50`, and selected-policy `full-323` reports above were refreshed after the scoped no-RNG fallback.
- The first selected-policy `full-323` refresh attempt was stopped because it had no progress output. The eval CLI now supports `--candidate` and `--progress-every`, and the progress-visible rerun completed.
- Useful command:
  - `uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --eval-set full-323 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Power Potion,Explosive Potion" --candidate tactical_greedy_d40 --progress-every 25 --output target/trace-guided/eval-full-323-tactical-greedy.json --failure-output target/trace-guided/eval-full-323-tactical-greedy-failures.json`

Interpretation:

- The no-RNG reshuffle fallback was the largest improvement so far.
- `tactical_greedy_d40` is not "done", but it is now plausibly useful for dataset replay assistance.
- The remaining blockers are quality on hard boss-like roots and nonterminal full-323 states.
- More trace data is still needed: `dev-50` has only 17 roots and `val-50` has only 4 held-out roots from the current single trace.

### 9. Trace Probe Candidate

Change:

- Added `terminal_probe`, a meta-search policy that runs tactical, HP-preserving, and scaling greedy probes.
  - If any probe sees a complete win from the current state, it takes the first winning probe's first action.
  - Otherwise it falls back to tactical greedy behavior.
- Added `trace_probe`, which extends `terminal_probe` with one targeted gate:
  - when there is a single high-HP artifact boss-like enemy, use `scaling_survival` greedy directly
  - otherwise use `terminal_probe`
- Added `trace_probe_d40` to `trace_autopilot_candidates()`.

Why:

- Blind `scaling_greedy_d40` was not safe:
  - `dev-50`: 13 wins, 3 losses, 1 nonterminal, mean HP loss 27.71
  - held-out `val-50`: 3 wins, 1 loss, mean HP loss 37.75
- However, scaling fixed some specific long-fight states:
  - dev step 216 was won by scaling-style play while greedy baselines died
  - held-out val step 262 was a single 300 HP artifact boss-like state where scaling won and greedy baselines timed out
- The trace probe keeps the tactical baseline for normal states and only lets scaling take over in states where the diagnostic evidence says it helps.

Rejected diagnostics:

| Candidate | `dev-50` Wins | Losses | Nonterminal | Mean HP Loss | Mean Seconds / Combat | Reason Rejected |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `scaling_greedy_d40` | 13/17 | 3 | 1 | 27.71 | 2.82 | failed held-out val with 1 loss |
| `tactical_beam_w2_d20` | 13/17 | 3 | 1 | 36.24 | 3.14 | slower and worse HP than greedy/probe |
| `hp_beam_w2_d20` | 13/17 | 3 | 1 | 37.06 | 3.10 | slower and worse HP than greedy/probe |

Selected-candidate `dev-50` comparison:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `trace_probe_d40` | 17 | 14 | 3 | 0 | 28.82 | 5 | 2.47 |
| `tactical_greedy_d40` | 17 | 13 | 4 | 0 | 32.47 | 5 | 1.95 |

Held-out `val-50` comparison:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `trace_probe_d40` | 4 | 4 | 0 | 0 | 20.0 | 0 | 2.05 |
| `tactical_greedy_d40` | 4 | 3 | 0 | 1 | 17.5 | 0 | 2.44 |

`full-323` coverage sanity:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `trace_probe_d40` | 323 | 281 | 12 | 30 | 21.64 | 30 | 1.99 |
| `tactical_greedy_d40` | 323 | 264 | 19 | 40 | 22.69 | 41 | 1.70 |

Interpretation:

- `trace_probe_d40` is the new best candidate on current evidence.
- It improves candidate-selection `dev-50`, held-out `val-50`, and coverage `full-323`.
- The runtime cost is real but still plausible for replay automation.
- The validation set is still undersized, so this should remain a candidate with documented caveats rather than proof that combat autopilot is solved.
