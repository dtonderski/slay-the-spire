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

### 10. No-Potion Trace Probe Candidate

Change:

- Added `trace_probe_no_potions_d40` as a first-class trace autopilot candidate.
- Updated trace eval candidate handling so explicit per-candidate potion constraints are not overwritten by a global `--allowed-potions` list.

Why:

- Several remaining dev failures spent potions and still died.
- A no-potion diagnostic showed almost identical combat quality on combat-start dev/val, much lower runtime, and zero potion waste.

No-potion `trace_probe` result:

| Eval Set | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Potion Uses | Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-fast-10` | 10 | 9 | 1 | 0 | 28.4 | 0 | 0.88 |
| `dev-50` | 17 | 14 | 3 | 0 | 28.94 | 0 | 1.44 |
| `val-50` | 4 | 4 | 0 | 0 | 20.0 | 0 | 1.24 |
| `full-323` | 323 | 280 | 23 | 20 | 21.94 | 0 | 1.16 |

Comparison against potion-allowed `trace_probe_d40`:

| Candidate | `dev-50` Wins | `val-50` Wins | `full-323` Wins | Full Losses | Full Nonterminal | Full Potion Uses | Full Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `trace_probe_d40` | 14/17 | 4/4 | 281/323 | 12 | 30 | 30 | 1.99 |
| `trace_probe_no_potions_d40` | 14/17 | 4/4 | 280/323 | 23 | 20 | 0 | 1.16 |

Rejected diagnostic:

- Book-of-Stabbing-specific `aggressive_beam_w2_d20` looked good on the dev combat-start Book root, but it regressed the broader Book subset:
  - `trace_probe_d40` on 21 Book roots: 16 wins, 5 losses, mean HP loss 55.19, 5 potion uses
  - `aggressive_beam_w2_d20` on 21 Book roots: 15 wins, 6 losses, mean HP loss 60.62, 6 potion uses
- Do not add a Book-wide beam gate without a narrower, better-validated condition.

Interpretation:

- `trace_probe_no_potions_d40` is not the selected strongest full-323 candidate because it loses one extra full root and has more full losses.
- It is still a useful replay/collection mode: same combat-start dev/val wins, zero potion consumption, and materially lower runtime.
- The next potion-aware policy should probably be a conditional two-pass policy: try no-potion first, then allow potions only when the no-potion rollout cannot win or leaves a clearly worse terminal result.

### 11. Potion-Rescue Trace Probe

Change:

- Added `potion_rescue_trace_probe`, a two-pass trace probe:
  - first search with `allowed_potions=()`
  - if that pass finds a terminal win, take the no-potion action
  - otherwise rerun `trace_probe` with the configured allowed potion list
- Added `trace_probe_potion_rescue_d40` as a first-class trace autopilot candidate.

Why:

- The no-potion probe tied the potion-allowed probe on combat-start dev/val, but lost one extra full-323 root and had more full-323 losses.
- The potion-allowed probe still used potions on some roots where no-potion already had a win.
- A conditional policy should preserve no-potion wins while retaining the current rescue path for hard roots.

Results:

| Eval Set | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Potion Uses | Mean Seconds / Combat | Mean Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-fast-10` | 10 | 9 | 1 | 0 | 28.2 | 19.0 | 68.95 | 2 | 1.29 | 1821.2 |
| `dev-50` | 17 | 14 | 3 | 0 | 28.82 | 23.0 | 76.0 | 5 | 2.16 | 2352.88 |
| `val-50` | 4 | 4 | 0 | 0 | 20.0 | 18.0 | 40.4 | 0 | 1.27 | 1462.25 |
| `full-323` | 323 | 281 | 12 | 30 | 21.63 | 14.0 | 64.0 | 29 | 1.42 | 1559.66 |

Comparison against current trace candidates:

| Candidate | `dev-50` Wins | `val-50` Wins | `full-323` Wins | Full Losses | Full Nonterminal | Full Potion Uses | Full Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `trace_probe_d40` | 14/17 | 4/4 | 281/323 | 12 | 30 | 30 | 1.99 |
| `trace_probe_potion_rescue_d40` | 14/17 | 4/4 | 281/323 | 12 | 30 | 29 | 1.42 |
| `trace_probe_no_potions_d40` | 14/17 | 4/4 | 280/323 | 23 | 20 | 0 | 1.16 |

Interpretation:

- Keep `trace_probe_potion_rescue_d40` as the current practical default candidate: same quality as `trace_probe_d40`, slightly less potion use, and much lower full-323 runtime.
- This does not solve the hard roots. The remaining `dev-50` losses are still the same difficult states around steps 91, 158, and 190.
- The next real quality iteration should target those failure fixtures directly; more generic potion gating mainly improves waste/runtime, not win rate.

### 12. Aggressive Rescue Trace Probe

Change:

- Added `aggressive_rescue_trace_probe`, a guarded rescue policy:
  - first run `potion_rescue_trace_probe`
  - if that pass finds a terminal win, keep it
  - otherwise run aggressive greedy search
  - take aggressive only if it finds a terminal win
- Added `trace_probe_aggressive_rescue_d40` as a first-class trace autopilot candidate.

Why:

- Direct failure-fixture probing showed:
  - Hexaghost step 91: trace, rescue, no-potion, scaling, aggressive, HP, and beam variants all still lost
  - Book of Stabbing step 158: aggressive greedy won with no potion where trace/rescue lost
  - Chosen+Cultist step 190: tested variants still lost
- The rescue is guarded so aggressive play cannot replace already-winning trace lines.

Results:

| Eval Set | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Potion Uses | Mean Seconds / Combat | Mean Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-50` | 17 | 15 | 2 | 0 | 28.06 | 23.0 | 73.4 | 4 | 2.26 | 2519.71 |
| `val-50` | 4 | 4 | 0 | 0 | 20.0 | 18.0 | 40.4 | 0 | 1.20 | 1481.75 |
| `full-323` | 323 | 291 | 7 | 25 | 21.29 | 14.0 | 63.9 | 18 | 1.43 | 1571.90 |

Comparison against the previous best:

| Candidate | `dev-50` Wins | `val-50` Wins | `full-323` Wins | Full Losses | Full Nonterminal | Full Potion Uses | Full Mean Seconds / Combat |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `trace_probe_potion_rescue_d40` | 14/17 | 4/4 | 281/323 | 12 | 30 | 29 | 1.42 |
| `trace_probe_aggressive_rescue_d40` | 15/17 | 4/4 | 291/323 | 7 | 25 | 18 | 1.43 |

Interpretation:

- `trace_probe_aggressive_rescue_d40` is the new best current policy on the frozen trace-derived evals.
- The improvement is objective and broad enough to keep: +1 win on candidate-selection `dev-50`, no held-out `val-50` regression, +10 wins on `full-323`, fewer losses, fewer nonterminals, and fewer potions.
- Remaining known dev losses are Hexaghost step 91 and Chosen+Cultist step 190. The quick fixture sweep did not find a Python-side policy variant that wins either, so the next iteration should inspect whether these need deeper Rust-side search, better long-horizon enemy modeling, or more specific simulator mechanic fixes.

### 13. Remaining Failure Diagnostics

Change:

- No policy change. Ran targeted diagnostics against the two remaining `dev-50` failure fixtures from `trace_probe_aggressive_rescue_d40`.

Why:

- The current best policy is materially better, but the remaining losses are large:
  - Hexaghost step 91: starts at 60 HP, spends Elixir and Cultist Potion, dies with Hexaghost still at 107 HP
  - Chosen+Cultist step 190: starts at 28 HP with Elixir and Distilled Chaos, dies with 24 monster HP remaining
- Before adding another guarded heuristic, verify that there is a local policy line worth rescuing.

Diagnostics:

| Diagnostic | Hexaghost Step 91 | Chosen+Cultist Step 190 | Decision |
| --- | --- | --- | --- |
| First-action sweep + current best finisher | all legal first actions still lost | all legal first actions still lost | rejected |
| Concrete Elixir exhaust lines | exhausting no cards, curses, and curse-plus-card variants still lost | exhausting no cards, individual Defends, and small Defend subsets still lost | rejected |
| Greedy finishers after Elixir lines | still lost | still lost | rejected |
| Heavy Python beam/portfolio probe | too slow to produce a practical result within the diagnostic window | too slow to produce a practical result within the diagnostic window | rejected as a Python replay policy path |

Interpretation:

- The remaining two `dev-50` losses do not look like simple first-action, potion-timing, Elixir-selection, or target-order mistakes under the current Python search shape.
- `trace_probe_aggressive_rescue_d40` remains the best practical candidate for replay automation.
- The next quality jump likely needs one of:
  - a Rust-side search core with cheaper cloning/rollout so deeper beams are practical
  - a more explicit long-horizon combat solver for boss/elite-like roots
  - simulator/mechanics inspection for these exact roots, especially Hexaghost and low-HP Chosen+Cultist
- Do not keep adding broad Python rescue passes until one of these diagnostics finds a winning line; otherwise runtime grows without evidence of better replay quality.

### 14. Potion Use Count Reporting

Change:

- Added per-potion use tracking to search episodes and trace-derived eval reports.
- Ranking rows now include `potion_use_counts`, alongside total and mean potion uses.
- Failure fixtures now include `potion_use_names` so hard roots preserve which potion types the candidate actually spent before failing.

Why:

- The objective evals need to distinguish "spent two potions" from "spent Elixir plus Cultist Potion".
- The current validation boundary uses the real trace potion types as an allowlist, so reports should make potion waste and potion-type dependence directly auditable.

Verification:

- `uv run python -m unittest python.tests.test_search_lab python.tests.test_self_play -v`
- Smoke eval on `dev-fast-10` with `trace_probe_aggressive_rescue_d40` wrote `potion_use_counts` in the ranking and `potion_use_names` in episode/failure rows.

Interpretation:

- This does not change policy behavior.
- It tightens the frozen eval/reporting layer before the next algorithm iteration, especially for comparing potion-aware candidates.

### 15. Real-Trace HP Baseline Reporting

Change:

- Added replay-derived real-trace HP baselines to trace eval roots.
- Episode rows now include `real_trace_final_hp`, `real_trace_hp_loss`, `real_trace_terminal_phase`, and `hp_loss_delta_vs_trace`.
- Ranking rows now include `mean_real_trace_hp_loss` and `mean_hp_loss_delta_vs_trace`.

Why:

- The search policy metrics need a stable comparison against the actual long trace, not only absolute simulator outcomes.
- This lets us see whether a policy is preserving more HP than the collected trace on the same combat-start root, while still tracking wins/losses separately.

Verification:

- `uv run python -m unittest python.tests.test_self_play -v`
- Smoke eval on `dev-fast-10` with `trace_probe_aggressive_rescue_d40` wrote real-trace baseline fields in episode and ranking rows.

Interpretation:

- The baseline is derived only from the replay trace summaries, not from external game state.
- Negative `real_trace_hp_loss` can happen when the trace exits combat with more HP than it had at the selected root, for example from post-combat healing. Treat `hp_loss_delta_vs_trace` as an HP-delta comparison, not as proof that the policy won the full combat unless `terminal_reason` also says it won.

### 16. Hard-Root Python Search Triage

Change:

- No policy change. Ran bounded probes after adding better eval reporting.

Why:

- The remaining `dev-50` losses are now the main quality blockers:
  - Hexaghost step 91
  - Chosen+Cultist step 190
- Before starting Rust-side search, check whether existing heavier Python candidates or cheaper first-action variants produce a winning line.

Diagnostics:

| Probe | Result | Decision |
| --- | --- | --- |
| `dev-fast-10` batch with `portfolio_aggressive_d40`, `beam_tactical_w8_d40`, `beam_aggressive_w12_d40`, `hp_portfolio_d40` | no ranking after 60s; process stopped | rejected as too slow for replay automation |
| two hard roots with `beam_tactical_w8_d40`, `beam_aggressive_w12_d40`, `hp_portfolio_d40` | no result after 30s; process stopped | rejected as too slow for Python-loop triage |
| hard roots with fast greedy policies | all lost both roots | no fast-policy rescue found |
| hard roots with no-potion variants | all lost both roots | potion waste is not the root cause |
| principal variation inspection | first-turn choices closely match the collected trace; failures appear later-horizon | points toward deeper search/horizon, not a first-action heuristic |

Fast hard-root results:

| Root | Candidate | Result | Final HP | Monster HP | Potions | Seconds |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| step 91 Hexaghost | `tactical_greedy_d40` | lost | -1 | 107 | 2 | 1.324 |
| step 91 Hexaghost | `hp_greedy_d40` | lost | -1 | 107 | 2 | 1.308 |
| step 91 Hexaghost | `aggressive_greedy_d40` | lost | -1 | 107 | 2 | 1.356 |
| step 91 Hexaghost | `scaling_greedy_d40` | lost | -10 | 118 | 2 | 1.099 |
| step 190 Chosen+Cultist | `tactical_greedy_d40` | lost | -13 | 24 | 2 | 1.061 |
| step 190 Chosen+Cultist | `hp_greedy_d40` | lost | -13 | 24 | 2 | 1.049 |
| step 190 Chosen+Cultist | `aggressive_greedy_d40` | lost | -13 | 23 | 2 | 1.052 |
| step 190 Chosen+Cultist | `scaling_greedy_d40` | lost | -1 | 52 | 2 | 0.820 |

No-potion hard-root results:

| Root | Candidate | Result | Final HP | Monster HP | Potions | Seconds |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| step 91 Hexaghost | `trace_probe_no_potions_d40` | lost | -3 | 107 | 0 | 1.231 |
| step 91 Hexaghost | `aggressive_no_potions_d40` | lost | -3 | 107 | 0 | 0.417 |
| step 91 Hexaghost | `scaling_no_potions_d40` | lost | -12 | 118 | 0 | 0.327 |
| step 190 Chosen+Cultist | `trace_probe_no_potions_d40` | lost | -13 | 31 | 0 | 1.134 |
| step 190 Chosen+Cultist | `aggressive_no_potions_d40` | lost | -13 | 30 | 0 | 0.401 |
| step 190 Chosen+Cultist | `scaling_no_potions_d40` | lost | -1 | 59 | 0 | 0.316 |

Interpretation:

- The Python policy stack is now evidence-limited on the remaining hard `dev-50` losses.
- The collected trace wins from these roots, so the simulator state is not obviously unwinnable.
- The first turn is not the obvious failure; the current policies need better long-horizon planning through later turns.
- The next meaningful quality iteration should move search closer to Rust or add a bounded Rust-side rollout primitive exposed to Python. More broad Python beam/portfolio attempts are likely to be too slow unless the candidate set and clone/step loop are drastically reduced.

### 17. First Rust-Side Greedy Rollout Primitive

Change:

- Added `OmniRunEnv.rust_greedy_combat_search(max_actions, objective, allowed_potions)` in the PyO3 crate.
- Added a Python-visible `RustSearchRecommendation` result with:
  - `best_action`
  - `value`
  - `actions`
  - `nodes`
  - `terminal_reason`
  - `final_hp`
  - `monster_hp`
- Exposed the result class through `sts.omni`.

Why:

- Broad Python beam/portfolio probes are too slow because every candidate repeatedly crosses the Python/Rust boundary, clones environments, serializes actions, and calls back into Python scoring.
- A Rust-side primitive proves the next architecture: keep rollout state, action enumeration, stepping, and simple scoring inside Rust, then expose only the recommendation summary to Python.

Verification:

- `cargo check -p py_sts`
- `uv run maturin develop --release`
- `uv run python -m unittest python.tests.test_run_omni_smoke -v`

Hard-root benchmark:

| Root | Objective | Terminal | Final HP | Monster HP | Actions | Nodes |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| step 91 Hexaghost | `tactical_survival` | nonterminal | 60 | 217 | 40 | 127 |
| step 91 Hexaghost | `aggressive_lethal` | nonterminal | 60 | 217 | 40 | 127 |
| step 91 Hexaghost | `hp_preserving_lethal` | nonterminal | 60 | 217 | 40 | 127 |
| step 190 Chosen+Cultist | `tactical_survival` | nonterminal | 28 | 78 | 40 | 134 |
| step 190 Chosen+Cultist | `aggressive_lethal` | nonterminal | 28 | 78 | 40 | 134 |
| step 190 Chosen+Cultist | `hp_preserving_lethal` | nonterminal | 28 | 78 | 40 | 134 |

Interpretation:

- This primitive is fast enough to be useful as a building block, but it is not strong enough to promote as an autopilot candidate.
- The first version is a one-step greedy rollout with simple Rust scoring. It does not yet do candidate-generation plus rollout, beam retention, terminal probing, or strong select-screen strategy.
- The next Rust-side search iteration should add a bounded beam or portfolio over this Rust rollout core, then compare it on `dev-fast-10` and `dev-50` before touching held-out `val-50`.

### 18. Rust Beam Search Candidate

Change:

- Added `OmniRunEnv.rust_beam_combat_search(max_actions, objective, allowed_potions, beam_width)`.
- Wired Python `search_combat` algorithms:
  - `rust_greedy`
  - `rust_beam`
- Added trace candidate names:
  - `rust_greedy_tactical_d40`
  - `rust_beam_tactical_w16_d40`

Why:

- The first Rust primitive proved that the Rust/PyO3 path is fast, but one-step greedy was not strong enough.
- A bounded Rust beam keeps multiple rollout states inside Rust and exposes only the selected first action to Python, avoiding the slow Python clone/step/score loop.

Results:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Potion Uses | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-fast-10` | `trace_probe_aggressive_rescue_d40` | 10 | 9 | 1 | 0 | 29.3 | 19.0 | 70.45 | 2 | 0.263 | 2507.9 |
| `dev-fast-10` | `rust_greedy_tactical_d40` | 10 | 9 | 1 | 0 | 17.2 | 4.5 | 51.9 | 12 | 0.002 | 828.3 |
| `dev-fast-10` | `rust_beam_tactical_w16_d40` | 10 | 8 | 0 | 2 | 18.3 | 3.5 | 58.15 | 8 | 0.017 | 10218.2 |
| `dev-50` | `trace_probe_aggressive_rescue_d40` | 17 | 14 | 2 | 1 | 26.88 | 23.0 | 63.0 | 4 | 0.380 | 2996.0 |
| `dev-50` | `rust_greedy_tactical_d40` | 17 | 13 | 3 | 1 | 26.35 | 11.0 | 79.2 | 16 | 0.003 | 1075.88 |
| `dev-50` | `rust_beam_tactical_w16_d40` | 17 | 13 | 0 | 4 | 11.41 | 1.0 | 53.6 | 10 | 0.025 | 10383.82 |

Hard-root notes from `dev-50`:

- `rust_beam_tactical_w16_d40` avoids outright losses on the known hard roots, but leaves several combats nonterminal at `max_actions=40`:
  - step 34: final HP 29, monster HP 28
  - step 91 Hexaghost: final HP 60, monster HP 217
  - step 158 Book of Stabbing: final HP 85, monster HP 131
  - step 190 Chosen+Cultist: final HP 28, monster HP 78

Interpretation:

- Do not promote `rust_beam_tactical_w16_d40` as the selected policy yet: on the candidate-selection set it wins fewer roots than the current best and leaves 4 nonterminals.
- Keep the Rust beam path. It is fast enough to iterate on and materially improves HP preservation, but it needs stronger terminal pressure, better long-horizon damage planning, and probably a guarded rescue composition with the current trace probe.
- The next Rust-side iteration should use `dev-50` only and try terminal-pressure variants before any held-out `val-50` comparison.

### 19. Terminal-Pressure Rust Beam

Change:

- Added a Rust-side `terminal_tactical` objective for beam search.
- The objective keeps the tactical survival terms, increases pressure against monster HP and live monsters, and applies a nonterminal penalty when the rollout reaches `max_actions` without ending combat.
- Added trace candidates:
  - `rust_beam_terminal_w16_d40`
  - `rust_beam_terminal_w32_d40`

Why:

- `rust_beam_tactical_w16_d40` preserved HP well, but often drifted into nonterminal rollouts with substantial monster HP remaining.
- The next hypothesis was that the beam needed stronger terminal pressure, not merely more width.

Results:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-fast-10` | `trace_probe_aggressive_rescue_d40` | 10 | 9 | 1 | 0 | 29.3 | 19.0 | 70.45 | 5.6 | 2 | 0.260 | 2507.9 |
| `dev-fast-10` | `rust_beam_tactical_w16_d40` | 10 | 8 | 0 | 2 | 18.3 | 3.5 | 58.15 | 5.6 | 8 | 0.016 | 10218.2 |
| `dev-fast-10` | `rust_beam_terminal_w16_d40` | 10 | 9 | 0 | 1 | 12.7 | 5.0 | 42.5 | 5.6 | 7 | 0.010 | 6313.2 |
| `dev-fast-10` | `rust_beam_terminal_w32_d40` | 10 | 8 | 0 | 2 | 9.5 | 2.0 | 39.5 | 5.6 | 3 | 0.023 | 13657.3 |
| `dev-50` | `trace_probe_aggressive_rescue_d40` | 17 | 14 | 2 | 1 | 26.88 | 23.0 | 63.0 | 6.35 | 4 | 0.395 | 2996.0 |
| `dev-50` | `rust_beam_tactical_w16_d40` | 17 | 13 | 0 | 4 | 11.41 | 1.0 | 53.6 | 6.35 | 10 | 0.023 | 10383.82 |
| `dev-50` | `rust_beam_terminal_w16_d40` | 17 | 15 | 0 | 2 | 15.94 | 7.0 | 59.6 | 6.35 | 11 | 0.015 | 6831.0 |
| `dev-50` | `rust_beam_terminal_w32_d40` | 17 | 14 | 0 | 3 | 10.35 | 3.0 | 39.4 | 6.35 | 7 | 0.027 | 12856.65 |

`dev-50` failure fixtures:

| Candidate | Trace Step | Result | Final HP | Monster HP | Potions |
| --- | ---: | --- | ---: | ---: | --- |
| `trace_probe_aggressive_rescue_d40` | 91 | lost | 0 | 62 | Cultist, Elixir |
| `trace_probe_aggressive_rescue_d40` | 190 | lost | -13 | 24 | DistilledChaos, Elixir |
| `trace_probe_aggressive_rescue_d40` | 216 | nonterminal | 48 | 10 | none |
| `rust_beam_tactical_w16_d40` | 34 | nonterminal | 29 | 28 | Elixir, Weak |
| `rust_beam_tactical_w16_d40` | 91 | nonterminal | 60 | 217 | none |
| `rust_beam_tactical_w16_d40` | 158 | nonterminal | 85 | 131 | none |
| `rust_beam_tactical_w16_d40` | 190 | nonterminal | 28 | 78 | DistilledChaos |
| `rust_beam_terminal_w16_d40` | 91 | nonterminal | 60 | 217 | none |
| `rust_beam_terminal_w16_d40` | 190 | nonterminal | 18 | 39 | DistilledChaos, Elixir |
| `rust_beam_terminal_w32_d40` | 34 | nonterminal | 80 | 89 | none |
| `rust_beam_terminal_w32_d40` | 69 | nonterminal | 73 | 90 | none |
| `rust_beam_terminal_w32_d40` | 158 | nonterminal | 85 | 131 | none |

Interpretation:

- `rust_beam_terminal_w16_d40` is the best current dev-50 candidate by wins: 15/17, zero losses, two nonterminals.
- It is not ready for held-out `val-50` promotion yet because the two remaining nonterminals are important hard roots, including Hexaghost at step 91 and Chosen+Cultist at step 190.
- Width 32 reduces HP loss and potion use, but wins fewer roots on dev-50 because it leaves more combats nonterminal. More width alone is not the next lever.
- The next iteration should target the remaining terminal failures directly: either compose terminal beam with a late rescue policy, add a lethal-finisher objective when monster HP is low, or extend Rust beam with rollout/portfolio selection instead of a single static objective.

### 20. Rust Terminal Portfolio

Change:

- Added `rust_terminal_portfolio_d40`.
- At each decision it asks three Rust-side beam candidates for a rollout:
  - `terminal_tactical`, width 16
  - `terminal_tactical`, width 32
  - `tactical_survival`, width 16
- The selector prefers predicted wins, then nonterminal rollouts with lower remaining monster HP, then final player HP.

Why:

- The terminal-pressure variants had complementary failure patterns.
- The first selector tried value-first tie-breaking and did not help: on `dev-50` it stayed at 15/17 and often preferred safer stalled lines.
- Switching nonterminal tie-breaking toward monster HP produced the intended late-combat pressure.

Results:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-fast-10` | `rust_beam_terminal_w16_d40` | 10 | 9 | 0 | 1 | 12.7 | 5.0 | 42.5 | 5.6 | 7 | 0.011 | 6313.2 |
| `dev-fast-10` | `rust_terminal_portfolio_d40` | 10 | 10 | 0 | 0 | 17.1 | 8.0 | 49.3 | 5.6 | 11 | 0.056 | 32461.0 |
| `dev-50` | `rust_beam_terminal_w16_d40` | 17 | 15 | 0 | 2 | 15.94 | 7.0 | 59.6 | 6.35 | 11 | 0.016 | 6831.0 |
| `dev-50` | `rust_terminal_portfolio_d40` | 17 | 16 | 1 | 0 | 19.06 | 7.0 | 54.0 | 6.35 | 15 | 0.081 | 35050.94 |

Remaining `dev-50` failure:

| Candidate | Trace Step | Result | Final HP | Monster HP | Potions |
| --- | ---: | --- | ---: | ---: | --- |
| `rust_terminal_portfolio_d40` | 190 | lost | 0 | 4 | DistilledChaos, Elixir |

Interpretation:

- This is the best current dev-50 policy by completion: 16/17 wins with no nonterminals, but the remaining failure is now an actual loss.
- It is also the first candidate to reach 10/10 on `dev-fast-10`.
- The tradeoff is real: it spends more potions and HP than the single terminal beam, and it is roughly 4-5x more expensive. The runtime is still small enough for replay automation on these roots.
- A Rust beam contract fix made this result stricter: the beam no longer returns `None` from a nonterminal root just because the root score beats all explored children. Before that fix, step 190 stalled as a nonterminal; after the fix, the policy continues and exposes the line as losing.
- Increasing the outer action cap from 40 to 80 did not change the result, so the remaining failure is not a simple action-budget problem.
- Disallowing Elixir is worse on `dev-50`: it drops to 15/17 wins with 2 losses, including Hexaghost.
- Do not touch held-out `val-50` yet. One known dev-50 root still fails: step 190 Chosen+Cultist. The next iteration should target that root specifically, ideally by improving first-turn/second-turn lethal planning rather than another broad static heuristic.

### 21. Width-32 Terminal Beam After Action-Contract Fix

Change:

- Re-ran the terminal Rust beam candidates after fixing the Rust beam action contract.
- The fix prevents the beam from returning a root placeholder with no first action when legal child actions exist. That made stalled nonterminal reports stricter and exposed which candidates actually continue playing.
- Selected `rust_beam_terminal_w32_d40` from `dev-50` before touching held-out validation.

Why:

- The portfolio looked best by completion before the action-contract fix, but it still lost the hard step 190 line.
- A direct hard-root probe showed that `terminal_tactical` width 32 could win that root after the action-contract fix, while the portfolio still selected a losing line.
- The selection rule remains dev-only: held-out `val-50` is for validation, not tuning.

Hard-root probe:

| Root | Candidate | Result | Final HP | Monster HP | Actions | Search Nodes |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| `dev-50` step 190 / state `eb21773a18409be3` | `rust_terminal_portfolio_d40` | lost | -16 | 4 | - | - |
| `dev-50` step 190 / state `eb21773a18409be3` | `rust_beam_terminal_w16_d40` | lost | -13 | 25 | - | - |
| `dev-50` step 190 / state `eb21773a18409be3` | `rust_beam_terminal_w32_d40` | won | 16 | 0 | 16 | 12921 |
| `dev-50` step 190 / state `eb21773a18409be3` | `rust_beam_terminal_w64_d40` | won | 16 | 0 | 18 | 26658 |
| `dev-50` step 190 / state `eb21773a18409be3` | aggressive width 64 diagnostic | won | 16 | 0 | 14 | 32005 |

Dev results after the action-contract fix:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-fast-10` | `rust_beam_terminal_w32_d40` | 10 | 10 | 0 | 0 | 17.1 | 7.5 | 51.2 | 5.6 | 7 | 0.00218 | 0.034 | 18224.7 |
| `dev-fast-10` | `rust_terminal_portfolio_d40` | 10 | 10 | 0 | 0 | 17.1 | 8.0 | 49.3 | 5.6 | 11 | 0.00316 | 0.052 | 32461.0 |
| `dev-50` | `rust_beam_terminal_w32_d40` | 17 | 17 | 0 | 0 | 19.29 | 12.0 | 57.6 | 6.35 | 12 | 0.00252 | 0.038 | 16245.35 |
| `dev-50` | `rust_terminal_portfolio_d40` | 17 | 16 | 1 | 0 | 19.06 | 7.0 | 54.0 | 6.35 | 15 | 0.00394 | 0.070 | 35050.94 |

Potion use on `dev-50` for `rust_beam_terminal_w32_d40`:

| Potion | Uses |
| --- | ---: |
| Cultist | 1 |
| DistilledChaos | 1 |
| Elixir | 5 |
| Explosive | 1 |
| Flex | 1 |
| Weak | 3 |

Held-out validation:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `val-50` | `rust_beam_terminal_w16_d40` | 4 | 4 | 0 | 0 | 10.0 | 8.5 | 18.35 | 4.0 | 1 | 0.00115 | 0.019 | 7421.5 |
| `val-50` | `rust_terminal_portfolio_d40` | 4 | 4 | 0 | 0 | 10.25 | 8.5 | 19.2 | 4.0 | 1 | 0.00522 | 0.119 | 44643.0 |
| `val-50` | `rust_beam_terminal_w32_d40` | 4 | 4 | 0 | 0 | 15.5 | 7.0 | 42.45 | 4.0 | 0 | 0.00336 | 0.065 | 19814.0 |

Important caveat:

- `val-50` is held out, but this trace currently contributes only 4 eval-split combat-start roots. Passing it is useful, not conclusive.
- Do not use the fact that width 16 ranked first on this tiny held-out slice to retune the policy. The selected policy remains `rust_beam_terminal_w32_d40` because it was the only compared candidate with 17/17 wins and zero nonterminals on `dev-50`.

Full coverage sanity:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `full-323` | `rust_beam_terminal_w32_d40` | 323 | 314 | 4 | 5 | 14.43 | 7.0 | 59.0 | -1.02 | 130 | 0.00182 | 0.029 | 11741.55 |

Full-set potion use:

| Potion | Uses |
| --- | ---: |
| Cultist | 1 |
| DistilledChaos | 17 |
| Elixir | 79 |
| Explosive | 3 |
| Flex | 1 |
| Power | 5 |
| Weak | 24 |

Full-set failure fixtures for the next iteration:

| Trace Step | Result | Final HP | Monster HP | Actions | Potions Used |
| ---: | --- | ---: | ---: | ---: | --- |
| 168 | lost | -14 | 21 | 17 | Elixir |
| 191 | lost | -16 | 4 | 15 | Elixir, DistilledChaos |
| 192 | lost | -16 | 4 | 14 | DistilledChaos, Elixir |
| 193 | lost | -10 | 12 | 14 | DistilledChaos, Elixir |
| 279 | nonterminal | 2 | 13 | 36 | Power |
| 284 | nonterminal | 1 | 65 | 16 | Power |
| 285 | nonterminal | 8 | 49 | 8 | Power |
| 286 | nonterminal | 8 | 61 | 6 | Power |
| 287 | nonterminal | 4 | 96 | 9 | Power |

Interpretation:

- `rust_beam_terminal_w32_d40` is now the selected baseline for replay automation experiments: it is fast, wins every dev-selected root, passes the small held-out validation set, and wins 314/323 full coverage roots.
- It is not finished. The full-set failures show two clusters: a mid-run lethal-planning cluster around steps 168 and 191-193, and a late cluster where Power Potion lines stall or leave too much monster HP.
- The policy spends many Elixirs on full coverage because the eval starts from many intermediate roots from the same real combat. This is useful for per-root combat advice, but it overstates whole-run potion consumption.
- Next iteration should target the saved full-set failure fixtures, especially the step 191-193 cluster and the Power Potion nonterminals, before claiming the autopilot is ready to drive large-scale trace replay.

### 22. Adaptive Width-128 No-Power Rescue

Change:

- Added trace candidates:
  - `rust_beam_terminal_w128_d40`
  - `rust_beam_terminal_w128_no_power_d40`
  - `rust_terminal_rescue_w32_w128_no_power_d40`
- Added the `rust_terminal_rescue` search algorithm.
- The rescue algorithm runs the current width-32 terminal beam first.
- If width 32 predicts a win, rescue returns that result unchanged.
- If width 32 predicts a loss or nonterminal, rescue runs a width-128 terminal beam with Power Potion removed from the allowed potion set.
- Rescue only selects the width-128 no-Power result when that result predicts a win; otherwise it keeps the width-32 result.

Why:

- Parallel failure inspection split the full-set failures into two clusters:
  - steps 168 and 191-193 were beam-pruning failures; wider terminal beams found winning lines.
  - steps 279 and 284-287 were Power Potion-heavy nonterminals; blindly banning Power Potion helped step 279 but turned steps 284-287 into losses.
- A blanket width-128 policy was worse than width 32 on `dev-fast-10` and `dev-50`: it was slower and gave up a little HP.
- A blanket no-Power policy was unsafe on the late cluster.
- The adaptive rule keeps the cheap dev-selected policy in normal states and only pays for wider search when the primary line is already predicted to fail.

Failure-fixture diagnostics before implementation:

| Trace Step | Width 32 | Width 64 | Width 128 | Width 128 No Power |
| ---: | --- | --- | --- | --- |
| 168 | lost, HP -14, monster 21 | won, HP 19 | won, HP 21 | won, HP 21 |
| 191 | lost, HP -16, monster 4 | won, HP 16 | won, HP 12 | won, HP 12 |
| 192 | lost, HP -16, monster 4 | lost, HP -16, monster 4 | won, HP 12 | won, HP 12 |
| 193 | lost, HP -10, monster 12 | lost, HP -10, monster 12 | won, HP 20 | won, HP 20 |
| 279 | nonterminal, HP 2, monster 13 | won, HP 8 | won, HP 22 | won, HP 22 |
| 284 | nonterminal, HP 1, monster 65 | nonterminal | nonterminal | lost, HP -6 |
| 285 | nonterminal, HP 8, monster 49 | nonterminal | nonterminal | lost, HP -6 |
| 286 | nonterminal, HP 8, monster 61 | nonterminal | nonterminal | lost, HP -8 |
| 287 | nonterminal, HP 4, monster 96 | nonterminal | nonterminal | lost, HP -3 |

Dev comparison:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-fast-10` | `rust_beam_terminal_w32_d40` | 10 | 10 | 0 | 0 | 17.1 | 7.5 | 51.2 | 5.6 | 7 | 0.00219 | 0.034 | 18224.7 |
| `dev-fast-10` | `rust_beam_terminal_w128_d40` | 10 | 10 | 0 | 0 | 18.7 | 13.0 | 48.5 | 5.6 | 7 | 0.00834 | 0.103 | 50308.3 |
| `dev-fast-10` | `rust_beam_terminal_w128_no_power_d40` | 10 | 10 | 0 | 0 | 18.7 | 13.0 | 48.5 | 5.6 | 7 | 0.00738 | 0.095 | 50308.3 |
| `dev-50` | `rust_beam_terminal_w32_d40` | 17 | 17 | 0 | 0 | 19.29 | 12.0 | 57.6 | 6.35 | 12 | 0.00248 | 0.037 | 16245.35 |
| `dev-50` | `rust_beam_terminal_w128_no_power_d40` | 17 | 17 | 0 | 0 | 19.82 | 15.0 | 57.6 | 6.35 | 10 | 0.00844 | 0.103 | 47857.29 |
| `dev-50` | `rust_terminal_rescue_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 19.24 | 12.0 | 56.8 | 6.35 | 12 | 0.00366 | 0.064 | 34442.82 |

Held-out validation:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `val-50` | `rust_beam_terminal_w32_d40` | 4 | 4 | 0 | 0 | 15.5 | 7.0 | 42.45 | 4.0 | 0 | 0.00334 | 0.064 | 19814.0 |
| `val-50` | `rust_terminal_rescue_w32_w128_no_power_d40` | 4 | 4 | 0 | 0 | 12.0 | 12.5 | 19.1 | 4.0 | 0 | 0.00359 | 0.061 | 22030.5 |

Full coverage sanity:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `full-323` | `rust_beam_terminal_w32_d40` | 323 | 314 | 4 | 5 | 14.43 | 7.0 | 59.0 | -1.02 | 130 | 0.00182 | 0.029 | 11741.55 |
| `full-323` | `rust_terminal_rescue_w32_w128_no_power_d40` | 323 | 318 | 1 | 4 | 14.09 | 8.0 | 55.9 | -1.02 | 124 | 0.00282 | 0.045 | 19591.73 |

Full-set potion use for rescue:

| Potion | Uses |
| --- | ---: |
| Cultist | 1 |
| DistilledChaos | 17 |
| Elixir | 75 |
| Explosive | 3 |
| Flex | 1 |
| Power | 4 |
| Weak | 23 |

Remaining full-set failure fixtures:

| Trace Step | Result | Final HP | Monster HP | Actions | Potions Used |
| ---: | --- | ---: | ---: | ---: | --- |
| 191 | lost | -16 | 4 | 15 | Elixir, DistilledChaos |
| 284 | nonterminal | 1 | 65 | 16 | Power |
| 285 | nonterminal | 8 | 49 | 8 | Power |
| 286 | nonterminal | 8 | 61 | 6 | Power |
| 287 | nonterminal | 4 | 96 | 9 | Power |

Interpretation:

- `rust_terminal_rescue_w32_w128_no_power_d40` is the new selected baseline from `dev-50`: it keeps 17/17 wins with zero nonterminals, slightly improves mean HP loss and p95 HP loss, and remains fast enough for replay automation.
- The full-set result improves from 314/323 wins to 318/323 wins, reducing losses from 4 to 1 and total unresolved fixtures from 9 to 5.
- The remaining step 191 loss is likely a repeated-decision/path issue: the single-root diagnostic showed width-128 no-Power can win that root, but the closed-loop policy still later drifts into the losing line.
- The remaining step 284-287 nonterminals need a Power Potion-specific evaluator or generated-card handling. Blanket Power Potion removal is unsafe because it turns those roots into losses.
- Next iteration should focus on Power Potion card-choice valuation and on preserving the step 191 winning line across subsequent closed-loop decisions.

### 23. Keyed Nonterminal Rescue and Combat Reward Legal Actions

Changes:

- Added `rust_terminal_rescue_keyed` and registered `rust_terminal_rescue_keyed_w32_w128_no_power_d40`.
- The keyed rescue still starts with the width-32 terminal beam and only runs the width-128 no-Power rescue when the primary line is not already a predicted win.
- Unlike `rust_terminal_rescue`, it can select a better nonterminal rescue line by comparing the same portfolio key used by the terminal portfolio. This fixes the step 191 closed-loop drift where the win-only guard rejected the Distilled Chaos first line.
- Fixed the Python/Rust omniscient legal-action adapter so open combat card rewards expose modal `ChooseCombatCardReward { index }` actions. This covers Power Potion, Discovery, and Toolbox style combat rewards.
- Added a smoke test that a combat card reward modal returns only `choose_combat_card_reward` actions.

Why:

- The step 191 failure was not a missing card implementation. Width-128 no-Power had a better nonterminal line, but `rust_terminal_rescue` only accepted rescue when it already predicted `won`.
- The Power Potion cluster was an adapter fidelity bug first: after using Power Potion, the core simulator opened valid choices, but `exact_run_legal_action_kinds` only exposed more potion actions. After rebuilding the PyO3 extension, the generated choices are visible.
- Once choices were visible, the same cluster became explicit losses instead of nonterminals. That is a fidelity improvement, not a solved policy.

Verification:

- `uv run maturin develop --release`
- Direct probe from step 284 after Power Potion now exposes:
  - `ChooseCombatCardReward { index: 0 }`
  - `ChooseCombatCardReward { index: 1 }`
  - `ChooseCombatCardReward { index: 2 }`
- `uv run python -m unittest discover -s python\tests -v`: 79 tests passed.
- A direct `cargo test --manifest-path simulator/Cargo.toml -p py_sts legal --lib` compiled but the pyo3 test binary failed to launch in this shell with `STATUS_DLL_NOT_FOUND`; the rebuilt Python extension tests are the authoritative smoke check for this path.

Rebuilt-extension evaluation:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Win Rate | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Mean Final HP | Mean Monster HP | Potion Uses | Mean Seconds / Decision | P50 Seconds / Decision | P95 Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-50` | `rust_terminal_rescue_keyed_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 1.000 | 19.71 | 12.0 | 57.6 | 6.35 | 54.35 | 0.00 | 12 | 0.00382 | 0.00196 | 0.02436 | 0.065 | 32032.06 | 91524.0 |
| `val-50` | `rust_terminal_rescue_keyed_w32_w128_no_power_d40` | 4 | 4 | 0 | 0 | 1.000 | 21.25 | 17.0 | 43.8 | 4.00 | 62.75 | 0.00 | 1 | 0.00354 | 0.00290 | 0.01210 | 0.057 | 18681.50 | 40279.1 |
| `full-323` | `rust_terminal_rescue_keyed_w32_w128_no_power_d40` | 323 | 319 | 4 | 0 | 0.988 | 14.55 | 8.0 | 58.9 | -1.02 | 52.53 | 0.78 | 152 | 0.00266 | 0.00155 | 0.01739 | 0.043 | 19242.89 | 83467.5 |

Full-set comparison after the legal-action fix:

| Candidate | Wins | Losses | Nonterminal | Mean HP Loss | P95 HP Loss | Potion Uses | Power Uses | Mean Seconds / Combat | Mean Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_terminal_rescue_keyed_w32_w128_no_power_d40` | 319 | 4 | 0 | 14.55 | 58.9 | 152 | 31 | 0.043 | 19242.89 |
| `rust_terminal_rescue_w32_w128_no_power_d40` | 317 | 6 | 0 | 14.70 | 59.0 | 151 | 31 | 0.044 | 19439.70 |
| `rust_beam_terminal_w32_d40` | 315 | 8 | 0 | 15.05 | 59.0 | 158 | 33 | 0.028 | 11580.01 |

Remaining full-set failures for keyed rescue:

| Trace Step | Result | Final HP | Monster HP | Actions | Potions Used |
| ---: | --- | ---: | ---: | ---: | --- |
| 284 | lost | -6 | 55 | 19 | Power |
| 285 | lost | -6 | 49 | 11 | Power |
| 286 | lost | -8 | 61 | 9 | Power |
| 287 | lost | -3 | 86 | 12 | Power |

Power-cluster diagnostics:

- Power Potion choices are `[Juggernaut, Rupture, Berserk]`.
- The policy chooses Juggernaut in all four remaining failures.
- Existing registry candidates and simple potion constraints do not win these roots, including width-128 no-Power, old rescue, terminal portfolio, no-potion trace probe, no-Power global, only Blessing, only Dexterity, and only Blessing plus Dexterity.
- Wider/deeper direct probes up to width 512 and depth 80 also lose all four roots, both with Power allowed and with Power removed.

Interpretation:

- The keyed rescue candidate remains the best current coverage candidate: it removes the step 191 loss and is better than the older rescue and plain width-32 after rebuilding the extension.
- The combat reward adapter fix is still important even though the four Power fixtures now become losses. The verifier/search can no longer silently stall with hidden generated-card choices.
- The remaining cluster is not solved by more beam width alone. It is likely a late Giant Head style tactical-ordering/search-quality issue around high incoming damage and delayed lethal, with Power Potion/Juggernaut being a symptom rather than a simple oracle.
- Next work should either add a stronger late-fight tactical objective/candidate that can value debuff-before-burst lines, or move the Rust beam toward principal-variation following / better rollout evaluation. Treat these four failures as the next fixed diagnostic set.

### 24. Combat-Start Scope Check and Rejected Debuff Objective

Change:

- Ran an explicit all-combat-start trace eval for the current selected candidate:
  - `uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --split all --root-scope combat_start --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Power Potion,Explosive Potion" --candidate rust_terminal_rescue_keyed_w32_w128_no_power_d40 --progress-every 10 --output target/trace-guided/eval-combat-start-all-rust-rescue-keyed-rebuilt.json --failure-output target/trace-guided/eval-combat-start-all-rust-rescue-keyed-rebuilt-failures.json`
- Probed an experimental Rust objective that:
  - estimated incoming damage after monster Weak
  - added short-term value for monster Weak and Vulnerable
  - otherwise kept the terminal tactical scoring shape
- Rejected that objective because it did not win any of the four remaining mid-combat failures, even with width 512 and depth 80.

All-combat-start result:

| Eval Scope | Candidate | Roots | Wins | Losses | Nonterminal | Win Rate | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Mean Final HP | Mean Monster HP | Potion Uses | Mean Seconds / Decision | P50 Seconds / Decision | P95 Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| all `combat_start` roots | `rust_terminal_rescue_keyed_w32_w128_no_power_d40` | 21 | 21 | 0 | 0 | 1.000 | 20.00 | 14.0 | 53.0 | 5.90 | 55.95 | 0.00 | 13 | 0.00359 | 0.00195 | 0.01999 | 0.061 | 29489.10 | 78045.0 |

Interpretation:

- For the current long trace, the selected candidate wins every combat-start root. This is the most relevant metric for dataset replay where the autopilot takes over at the beginning of combat.
- The remaining `full-323` losses are arbitrary mid-combat recovery states, not combat starts. They are still useful stress tests, but they should not be treated as evidence that the current combat-start replay loop cannot work.
- The failed debuff objective is evidence that the step 284-287 cluster is not solved by simply valuing Weak/Vulnerable or weak-adjusted incoming damage. Those states may require a stronger recovery planner, explicit principal-variation following, or accepting that some mid-combat trace states are already losing under the simulator branch.

### 25. Frozen Root Manifest and Top-Candidate Validation Pass

Change:

- Added a top-level `root_manifest` to trace eval JSON reports. Each report now records the exact trace path, trace step, state ID, split, potion inventory, legal action kinds, legal potion names, allowed-potion availability, and real-trace HP loss for every selected root.
- This makes the named eval sets auditable as frozen trace-derived selections instead of only count-based reports.
- Re-ran the current top candidates using the trace potion allowlist:
  - `Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Power Potion,Explosive Potion`

Candidate-selection pass on `dev-50`:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_beam_terminal_w32_d40` | 17 | 17 | 0 | 0 | 19.47 | 57.6 | 6.35 | 12 | 0.00260 | 0.039 | 16230.5 |
| `rust_terminal_rescue_keyed_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 19.71 | 57.6 | 6.35 | 12 | 0.00338 | 0.058 | 32032.1 |
| `rust_beam_terminal_w128_no_power_d40` | 17 | 17 | 0 | 0 | 19.82 | 57.6 | 6.35 | 10 | 0.00842 | 0.104 | 47857.3 |
| `rust_beam_terminal_w128_d40` | 17 | 17 | 0 | 0 | 19.82 | 57.6 | 6.35 | 10 | 0.00846 | 0.103 | 48237.0 |
| `rust_terminal_portfolio_d40` | 17 | 16 | 1 | 0 | 19.06 | 54.0 | 6.35 | 16 | 0.00399 | 0.071 | 35018.6 |

Held-out validation for the best dev candidates:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_beam_terminal_w128_no_power_d40` | 4 | 4 | 0 | 0 | 14.25 | 20.0 | 4.00 | 1 | 0.00826 | 0.124 | 37320.5 |
| `rust_terminal_rescue_keyed_w32_w128_no_power_d40` | 4 | 4 | 0 | 0 | 21.25 | 43.8 | 4.00 | 1 | 0.00311 | 0.051 | 18681.5 |
| `rust_beam_terminal_w32_d40` | 4 | 4 | 0 | 0 | 24.75 | 48.0 | 4.00 | 1 | 0.00287 | 0.054 | 16465.0 |

Full coverage sanity for the validation-favored candidate:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Win Rate | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Mean Final HP | Mean Monster HP | Potion Uses | Mean Seconds / Decision | P50 Seconds / Decision | P95 Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `full-323` | `rust_beam_terminal_w128_no_power_d40` | 323 | 319 | 4 | 0 | 0.988 | 15.26 | 10.0 | 59.9 | -1.02 | 51.82 | 0.84 | 113 | 0.00655 | 0.00543 | 0.03456 | 0.086 | 34029.46 | 123196.4 |

Remaining full-set failures for `rust_beam_terminal_w128_no_power_d40`:

| Trace Step | Result | Final HP | Monster HP | Actions | Potions Used |
| ---: | --- | ---: | ---: | ---: | --- |
| 284 | lost | -6 | 65 | 16 | none |
| 285 | lost | -6 | 49 | 8 | none |
| 286 | lost | -8 | 61 | 6 | none |
| 287 | lost | -3 | 96 | 9 | none |

Interpretation:

- `rust_beam_terminal_w32_d40` is the best `dev-50` candidate by the current ranking tie-breaks and is the fastest of the zero-failure dev candidates.
- The held-out `val-50` roots strongly favor `rust_beam_terminal_w128_no_power_d40` on HP preservation, but the validation set is only four roots from this single trace, so this is evidence for keeping it as a top candidate rather than enough evidence to replace every default.
- `rust_beam_terminal_w128_no_power_d40` uses fewer potions on `full-323` than keyed rescue and wins the same 319/323 roots, but it has worse full-set HP loss, worse p95 HP loss, and roughly double the runtime.
- The persistent step 284-287 losses remain a mid-combat recovery problem, not a combat-start replay problem. More beam width alone has not solved them.

### 26. Winning-Line HP Selector

Change:

- Added `rust_terminal_win_hp_selector_w32_w128_no_power_d40`.
- The selector runs:
  - width-32 `terminal_tactical` with the configured potion allowlist
  - width-128 `terminal_tactical` with Power Potion removed from the allowlist
- It does not change Rust scoring. Instead, it selects between completed Rust recommendations with the existing terminal-first portfolio key, so HP only breaks ties after terminal success is already secured.

Why:

- A trial Rust `hp_preserving_lethal` terminal-pressure objective regressed `dev-50` to 16/17 wins. That made direct HP-pressure scoring unsafe.
- The safer next shape is to compare already-winning lines rather than steer beam expansion with a new objective.
- This is intentionally a candidate, not a global replacement for every UI/search path, because it is slower than width-32 and does not solve the four mid-combat Giant Head recovery failures.

Candidate-selection pass on `dev-50`:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 17.24 | 12.0 | 57.6 | 6.35 | 12 | 0.00942 | 0.127 | 64957.2 | 177492.0 |
| `rust_beam_terminal_w32_d40` | 17 | 17 | 0 | 0 | 19.47 | 12.0 | 57.6 | 6.35 | 12 | 0.00243 | 0.036 | 16230.5 | 49352.0 |
| `rust_beam_terminal_w128_no_power_d40` | 17 | 17 | 0 | 0 | 19.82 | 15.0 | 57.6 | 6.35 | 10 | 0.00804 | 0.098 | 47857.3 | 124954.6 |

Held-out validation:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_beam_terminal_w128_no_power_d40` | 4 | 4 | 0 | 0 | 14.25 | 17.0 | 20.0 | 4.00 | 1 | 0.00913 | 0.134 | 37320.5 | 85516.7 |
| `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 4 | 4 | 0 | 0 | 14.25 | 17.0 | 20.0 | 4.00 | 1 | 0.01113 | 0.178 | 62545.8 | 137309.1 |
| `rust_beam_terminal_w32_d40` | 4 | 4 | 0 | 0 | 24.75 | 25.5 | 48.0 | 4.00 | 1 | 0.00269 | 0.050 | 16465.0 | 39495.8 |

Full coverage sanity:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Win Rate | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Mean Final HP | Mean Monster HP | Potion Uses | Mean Seconds / Decision | P50 Seconds / Decision | P95 Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `full-323` | `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 323 | 319 | 4 | 0 | 0.988 | 13.58 | 8.0 | 56.0 | -1.02 | 53.50 | 0.78 | 151 | 0.00818 | 0.00728 | 0.04555 | 0.119 | 46462.44 | 176371.9 |

Remaining full-set failures:

| Trace Step | Result | Final HP | Monster HP | Actions | Potions Used |
| ---: | --- | ---: | ---: | ---: | --- |
| 284 | lost | -6 | 55 | 19 | Power |
| 285 | lost | -6 | 49 | 11 | Power |
| 286 | lost | -8 | 61 | 9 | Power |
| 287 | lost | -3 | 86 | 12 | Power |

Interpretation:

- The selector is the best current `dev-50` candidate among the tested terminal Rust policies and improves full-set HP loss versus the previous keyed-rescue baseline.
- Held-out `val-50` ties `rust_beam_terminal_w128_no_power_d40` on win rate and HP preservation, while costing more runtime. With only four held-out combat-start roots, this is not enough to remove the cheaper candidate; both should remain available.
- The candidate is practical for replay automation: the full-set mean is about 0.119 search seconds per combat, with p95 decision latency about 0.046 seconds.
- The step 284-287 failures persist unchanged and should be treated as a separate mid-combat recovery/horizon problem.

### 27. Bounded Rust Terminal Rollout Selector

Change:

- Added `rust_terminal_rollout_selector_w32_w128_no_power_d40`.
- The candidate starts from the same two beams as `rust_terminal_win_hp_selector_w32_w128_no_power_d40`:
  - width-32 `terminal_tactical` with the configured potion allowlist
  - width-128 `terminal_tactical` with Power Potion removed from the allowlist
- If the selected Rust beam is a high-HP terminal win, it returns the existing selector result.
- If the selected Rust beam is nonterminal, lost, or wins below 20 HP, it replays each unique candidate first action and runs an 8-action continuation rollout using the same two Rust beam policies. The continued line is selected with an HP-preserving terminal outcome score.

Why:

- The previous selector improved HP preservation without changing Rust scoring, but it still had horizon optimism on low-HP/mid-combat recovery states.
- This prototype keeps the search bounded and Python-only while asking a more concrete question: "after this first action, can the same policy family actually finish the fight from the resulting state?"
- This was tuned only on `dev-fast-10` and `dev-50`; held-out `val-50` was not used for candidate selection.

Dev-fast sanity:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | P95 HP Loss | Potion Uses | Mean Seconds / Combat | Mean Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_terminal_rollout_selector_w32_w128_no_power_d40` | 10 | 10 | 0 | 0 | 15.80 | 40.75 | 7 | 0.354 | 204564.3 |
| `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 10 | 10 | 0 | 0 | 15.90 | 41.30 | 7 | 0.139 | 74461.6 |

Candidate-selection pass on `dev-50`:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Real Trace Mean HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_terminal_rollout_selector_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 17.18 | 12.0 | 56.8 | 6.82 | 11 | 0.02279 | 0.381 | 206701.5 | 832561.2 |
| `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 17.24 | 12.0 | 57.6 | 6.82 | 12 | 0.01071 | 0.143 | 64957.2 | 177492.0 |

Interpretation:

- The continuation candidate preserved the `dev-50` win rate and very slightly improved mean and p95 HP loss.
- The runtime and node cost are much worse, so this is not an obvious replacement for the committed selector.
- The useful next implementation shape is to make the continuation trigger narrower or target known recovery roots, then run `full-323` only if the dev advantage becomes larger than noise.

### 28. Selected Policy Wired Into Replay and UI

Change:

- Promoted `rust_terminal_win_hp_selector_w32_w128_no_power_d40` into a named selected combat autopilot constant.
- `sts.self_play run` and `sts.self_play batch` now default to that selected candidate and expose `--combat-policy` for overrides.
- The local UI search panel now defaults to the selected policy, exposes the top practical/experimental candidates by name, and sends the selected candidate to `/search`.
- `/search` accepts named candidates, max-depth/objective/algorithm/beam overrides, and an explicit potion allowlist.
- Rust-backed policies fall back to the Python beam path only for legacy in-memory env objects that do not expose Rust search methods; real run/replay envs still exercise the Rust policy.

Why:

- The selector is the current best practical default from the frozen trace-derived evals: it preserves the zero-failure combat-start behavior, improves HP loss versus width-32 on `dev-50`, and keeps full-set runtime acceptable.
- Keeping the other top candidates selectable lets the UI act as a comparison/debug surface without changing the replay default.
- The fallback prevents old UI fixture sessions from failing noisily while keeping diagnostics explicit (`rust_search_unavailable`, `fallback_algorithm`).

Validation:

```powershell
uv run python -m unittest python.tests.test_ui_service python.tests.test_self_play python.tests.test_search_lab python.tests.test_search_smoke -v

uv run python -m sts.self_play eval `
  --trace target/trace-guided/manual01-replayed.jsonl `
  --root-scope combat_start `
  --split all `
  --candidate rust_terminal_win_hp_selector_w32_w128_no_power_d40 `
  --max-actions 40 `
  --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" `
  --output target/trace-guided/eval-combat-start-all-win-hp-selector-ui-default.json `
  --failure-output target/trace-guided/eval-combat-start-all-win-hp-selector-ui-default-failures.json
```

Focused tests: 59 passed.

Combat-start-all selected-policy gate:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Win Rate | Mean HP Loss | Median HP Loss | P95 HP Loss | Mean Final HP | Mean Monster HP | Potion Uses | Mean Seconds / Decision | P50 Seconds / Decision | P95 Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `combat_start/all` | `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 21 | 21 | 0 | 0 | 1.000 | 16.67 | 14.0 | 53.0 | 59.29 | 0.0 | 13 | 0.01077 | 0.00931 | 0.03146 | 0.148 | 64497.9 | 171024.0 |

Interpretation:

- The selected policy is now usable as the default combat autopilot in replay/self-play and UI search.
- The combat-start replay gate is clean: every combat-start root from `manual01-replayed` wins with no nonterminal failures.
- Full-323 still has the known four mid-combat Giant Head recovery losses from section 26; those remain a separate horizon/recovery milestone, not a blocker for combat-start replay automation.

### 29. Low-HP Recovery Rollout Candidate

Change:

- Added `rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40`.
- The candidate starts from the same two Rust beams as `rust_terminal_win_hp_selector_w32_w128_no_power_d40`.
- It only attempts a continuation rollout when the selected line is lost, nonterminal, or reports a terminal win at 8 HP or lower.
- The continuation rollout is intentionally narrow: width-32 Rust beam, capped at 4 additional actions.
- Failure fixtures now include a per-decision `decision_trace` with selected action JSON, selector candidates, Rust final HP/monster HP, nodes, timing, and fallback diagnostics.

Why:

- The known full-set failures are one mid-combat recovery cluster, not normal combat-start failures.
- Previous broad rollout improved HP slightly but was too expensive to promote. This tries the same idea with a much narrower trigger and cheaper continuation.
- Richer failure diagnostics let future experiments distinguish "the first action was bad" from "the selected line looked survivable but later collapsed."

Validation:

```powershell
uv run python -m unittest python.tests.test_search_smoke python.tests.test_search_lab python.tests.test_self_play -v

uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --eval-set dev-50 --candidate rust_terminal_win_hp_selector_w32_w128_no_power_d40 --candidate rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" --output target/trace-guided/eval-dev-50-low-hp-rollout.json --failure-output target/trace-guided/eval-dev-50-low-hp-rollout-failures.json

uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --eval-set val-50 --candidate rust_terminal_win_hp_selector_w32_w128_no_power_d40 --candidate rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" --output target/trace-guided/eval-val-50-low-hp-rollout.json --failure-output target/trace-guided/eval-val-50-low-hp-rollout-failures.json

uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --eval-set full-323 --candidate rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" --output target/trace-guided/eval-full-323-low-hp-rollout.json --failure-output target/trace-guided/eval-full-323-low-hp-rollout-failures.json
```

Focused tests: 40 passed.

Dev-50:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 17.18 | 12.0 | 56.8 | 11 | 0.01146 | 0.164 | 84605.2 | 243790.0 |
| `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 17 | 17 | 0 | 0 | 17.24 | 12.0 | 57.6 | 12 | 0.01093 | 0.146 | 64957.2 | 177492.0 |

Held-out `val-50`:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40` | 4 | 4 | 0 | 0 | 14.25 | 17.0 | 20.0 | 1 | 0.01066 | 0.172 | 62545.8 | 137309.1 |
| `rust_terminal_win_hp_selector_w32_w128_no_power_d40` | 4 | 4 | 0 | 0 | 14.25 | 17.0 | 20.0 | 1 | 0.01369 | 0.208 | 62545.8 | 137309.1 |

Full coverage sanity:

| Eval Set | Candidate | Roots | Wins | Losses | Nonterminal | Win Rate | Mean HP Loss | Median HP Loss | P95 HP Loss | Mean Final HP | Mean Monster HP | Potion Uses | Mean Seconds / Decision | P50 Seconds / Decision | P95 Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `full-323` | `rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40` | 323 | 319 | 4 | 0 | 0.988 | 13.66 | 8.0 | 55.8 | 53.42 | 0.78 | 149 | 0.00865 | 0.00741 | 0.04585 | 0.128 | 51733.8 | 190892.4 |

Remaining full-set failures for the low-HP rollout candidate:

| Trace Step | State ID | Split | Result | Final HP | Monster HP | Actions | Potions Used |
| ---: | --- | --- | --- | ---: | ---: | ---: | --- |
| 286 | `1fa07cf0a648af57` | dev | lost | -6 | 55 | 19 | Power |
| 287 | `ac65a0d4bd9a5f9a` | dev | lost | -6 | 49 | 8 | none |
| 288 | `ae92813c259ea269` | eval | lost | -8 | 61 | 6 | none |
| 289 | `7997bc68c3ac7354` | eval | lost | -3 | 86 | 12 | Power |

Interpretation:

- The narrow rollout is a valid experimental candidate, but it does not solve the recovery cluster.
- It slightly improves `dev-50` HP/potion metrics and slightly improves full-set p95 HP loss/potion use, but costs more nodes and full-set runtime than the selected default.
- `val-50` ties the selected default on all outcome/HP/potion metrics.
- Do not promote this candidate. Keep it available for diagnostics and future targeted recovery experiments.
- The next useful direction is likely not another shallow rollout wrapper. The failure fixtures suggest either a truly doomed reconstructed state, trace/replay drift around the same fight, or a need for a bounded oracle/branch-and-bound solver over this exact cluster.

### 30. Failure-Cluster Oracle And Trace Fidelity Check

Change:

- Added `sts.search_lab oracle-failures`, a bounded diagnostic oracle over saved failure fixtures.
- The oracle loads exact fixture `snapshot_json`, explores simulator legal actions directly with memoization, applies the same optional potion allowlist shape as trace evals, and reports whether a win was found before the node/action cap.
- This is diagnostic tooling only. It is not a replay policy and is not used by UI/default autopilot.

Why:

- The four remaining `full-323` losses are all mid-combat slices of the same late fight, so another candidate wrapper is unlikely to explain whether they are solvable.
- A bounded oracle can separate "beam/search missed an obvious winning branch" from "the reconstructed state is probably not recoverable under current simulator fidelity."
- Independent trace inspection found Reaper was not lost: the raw trace plays Reaper before the failure roots, HP rises, and Reaper is in exhaust afterward. However, the reconstructed replay/failure snapshots have `relics: []` while the raw trace around the same fight has many relics, including Burning Blood, Eternal Feather, Pocketwatch, Pear, Frozen Egg, Champion Belt, Golden Idol, Du-Vu Doll, Mark of Pain, Medical Kit, War Paint, Letter Opener, and Stone Calendar. So the snapshots are coherent combat skeletons, but not faithful full game states.

Validation:

```powershell
uv run python -m unittest python.tests.test_search_lab python.tests.test_self_play -v

uv run python -m sts.search_lab oracle-failures `
  target/trace-guided/eval-full-323-win-hp-selector-diagnostics-failures.json `
  --max-actions 16 `
  --max-nodes 50000 `
  --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" `
  --output target/trace-guided/oracle-full-323-win-hp-selector-failures.json
```

Focused tests: 18 passed.

Oracle result:

| Trace Step | Split | Win Found | Exhausted | Nodes | Best Terminal | Best Final HP | Best Monster HP | Best Actions |
| ---: | --- | --- | --- | ---: | --- | ---: | ---: | --- |
| 286 | dev | no | no | 50000 | nonterminal | 59 | 161 | 3 |
| 287 | dev | no | no | 50000 | nonterminal | 59 | 118 | 2 |
| 288 | eval | no | no | 50000 | nonterminal | 59 | 118 | 1 |
| 289 | eval | no | yes | 49086 | nonterminal | 59 | 151 | 0 |

Interpretation:

- The oracle found `0/4` wins under the current reconstructed no-relic snapshots.
- Three fixtures hit the `50000` node cap, so this is not a proof of impossibility for those roots.
- The last fixture exhausted under the configured `16` action limit without finding a win.
- Combined with missing relic continuity, the next milestone should be state repair/import fidelity, not more policy tuning. Specifically: preserve/import real relic inventory and relevant relic counters into anchored replay snapshots, rerun `full-323`, and only return to policy work if the repaired failure roots still lose.

### 31. Observed Combat Relic Import Repair

Change:

- Repaired `run_state_from_observed_combat_message` so CommunicationMod combat anchors import observed relic inventory instead of only `Mummified Hand` and `Pen Nib`.
- Added trace-name normalization for CommunicationMod variants such as `Frozen Egg 2` and `StoneCalendar`.
- Imported supported relic counters into `CombatState.relic_counters`, including Pen Nib, Nunchaku, Letter Opener, Pocketwatch turn-card count, Stone Calendar turn count, Happy Flower, Ink Bottle, Shuriken, Kunai, and Incense Burner.
- Inferred `energy_per_turn`/`max_energy` from observed energy boss relics such as Mark of Pain.
- Added a defensive Python snapshot enrichment/relic-count drift check for trace-guided anchors. The Rust importer is the source-of-truth fix; the Python layer prevents stale or partially imported anchors from silently retaining empty relic lists.

Why:

- The previous four `full-323` losses were all from a mid-combat Giant Head recovery cluster.
- Raw MANUAL01 states around that cluster had 13 relics, including Pocketwatch, Champion Belt, Mark of Pain, Medical Kit, Letter Opener, and Stone Calendar.
- The replay snapshots used by eval had `state.relics: []` and `combat.relics: []`, so the policy was being asked to recover from an unfair, underpowered state.

Validation:

```powershell
cargo test -p sts_verify
uv run maturin develop --release
uv run python -m sts.self_play replay-real-trace --trace ..\verification\corpus\communication_mod\trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl --output target\trace-guided\manual01-replayed.jsonl --report-output target\trace-guided\manual01-report.json
uv run python -m unittest python.tests.test_search_lab python.tests.test_self_play python.tests.test_search_smoke -v

uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --eval-set dev-50 --candidate rust_terminal_win_hp_selector_w32_w128_no_power_d40 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" --output target/trace-guided/eval-dev-50-win-hp-selector-relic-repaired.json --failure-output target/trace-guided/eval-dev-50-win-hp-selector-relic-repaired-failures.json

uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --eval-set val-50 --candidate rust_terminal_win_hp_selector_w32_w128_no_power_d40 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" --output target/trace-guided/eval-val-50-win-hp-selector-relic-repaired.json --failure-output target/trace-guided/eval-val-50-win-hp-selector-relic-repaired-failures.json

uv run python -m sts.self_play eval --trace target/trace-guided/manual01-replayed.jsonl --eval-set full-323 --candidate rust_terminal_win_hp_selector_w32_w128_no_power_d40 --max-actions 40 --allowed-potions "Weak Potion,Cultist Potion,Flex Potion,Elixir,Distilled Chaos,Explosive Potion,Power Potion" --progress-every 25 --output target/trace-guided/eval-full-323-win-hp-selector-relic-repaired.json --failure-output target/trace-guided/eval-full-323-win-hp-selector-relic-repaired-failures.json
```

Test results:

- `cargo test -p sts_verify`: 72 unit tests and 25 corpus tests passed.
- Python focused tests: 41 passed.
- Regenerated replay: verified, `trace_exhausted`, 327 steps, 326 extractable combat roots.
- Spot-check around raw trace steps 476-486 now shows the 13 observed relics in both run and combat snapshots, with Mark of Pain raising energy per turn to 4.

Repaired selected-policy results:

| Eval Set | Roots | Wins | Losses | Nonterminal | Win Rate | Mean HP Loss | Median HP Loss | P95 HP Loss | Mean Final HP | Mean Monster HP | Potion Uses | Mean Seconds / Decision | P50 Seconds / Decision | P95 Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes | P95 Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `dev-50` | 16 | 16 | 0 | 0 | 1.000 | 11.88 | 4.0 | 53.25 | 65.25 | 0.0 | 8 | 0.01710 | 0.01438 | 0.06782 | 0.254 | 90802.9 | 228390.8 |
| `val-50` | 5 | 5 | 0 | 0 | 1.000 | 7.60 | 5.0 | 20.80 | 64.60 | 0.0 | 4 | 0.01524 | 0.01317 | 0.03965 | 0.189 | 66158.6 | 162585.8 |
| `full-323` | 323 | 323 | 0 | 0 | 1.000 | 9.73 | 7.0 | 39.80 | 57.39 | 0.0 | 104 | 0.01100 | 0.01045 | 0.05791 | 0.132 | 46211.5 | 212966.0 |

Full-set potion use after repair:

| Potion | Uses |
| --- | ---: |
| Cultist | 1 |
| Distilled Chaos | 17 |
| Elixir | 59 |
| Explosive | 1 |
| Flex | 1 |
| Power | 2 |
| Weak | 23 |

Interpretation:

- The previous `full-323` failure cluster was an import-fidelity bug, not evidence that the selected search policy could not recover the fight.
- After relic repair and replay regeneration, the selected default policy wins every frozen `full-323` root with zero failure fixtures.
- The named combat-start sets changed size because the repaired replay now exposes 326 extractable combat roots; current MANUAL01 contributes 16 dev combat-start roots and 5 held-out validation combat-start roots.
- The validation set is still too small to claim broad generalization. More real traces remain useful, but there is no longer a known MANUAL01 combat-autopilot blocker.

### 32. Trace-Used Potions and HP-Plan Selector

Change:

- Fixed trace-guided replay parsing for CommunicationMod commands shaped like `POTION USE <slot> [target]`.
- Added conservative potion command fallbacks for targetless potion actions and compacted observed slots.
- Added `allowed_potions_mode="trace_used"` for trace evaluation. In this mode each combat root only allows the potion names the real trace later used in that same combat window.
- Exposed Rust search principal variations through `RustSearchRecommendation.principal_variation`.
- Added `rust_terminal_hp_selector_w32_w64_w128_d40`.
- Tried a committed principal-variation-following variant; after fixing its diagnostics flag, it introduced a dev-set loss and was not retained as a named lab candidate.
- Kept Power Potion out of speculative HP-preserving/high-width selector branches after a Power-generated line produced one full-set loss.

Why:

- The previous full-set eval used a global potion allowlist and therefore let the policy use many potions the real trace did not spend in that combat.
- The regenerated replay previously mapped zero simulator `use_potion` steps even though the raw trace had seven `POTION USE` commands.
- The current UI plan is for a human to choose whether potions are allowed per combat; trace eval should mirror that boundary by using the trace's own potion decisions only.

Validation:

```powershell
uv run maturin develop --release
uv run python -m unittest python.tests.test_self_play python.tests.test_search_smoke -v
uv run python -m sts.self_play replay-real-trace --trace ..\verification\corpus\communication_mod\trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl --output target\trace-guided\manual01-replayed.jsonl --report-output target\trace-guided\manual01-report.json
uv run python -m sts.self_play eval --trace target\trace-guided\manual01-replayed.jsonl --eval-set full-323 --max-actions 80 --allowed-potions-mode trace_used --candidate rust_terminal_hp_selector_w32_w64_w128_d40 --output target\search-lab\trace-used-full323-hp-selector-nopower.json
```

Results:

- Focused Python tests: 38 passed.
- Regenerated MANUAL01 replay: verified, `trace_exhausted`, 334 steps, 326 extractable roots.
- Potion replay repair: all seven raw trace potion commands are now simulator `use_potion` steps; potion restorations are zero.
- Honest selected-policy baseline with trace-used potions on `full-323`: 323 wins, 0 losses, 0 nonterminal, mean HP loss 10.18, median 7.0, p95 42.0, 32 potion uses.
- HP selector before Power safety: mean HP loss 6.79, but 1 loss from a Power Potion line.
- HP selector after Power safety: 323 wins, 0 losses, 0 nonterminal, mean HP loss 6.88, median 4.0, p95 36.0, 36 potion uses, mean seconds per decision 0.0233.
- Principal-variation-following experiment after the diagnostics flag fix: on `dev-50`, 15 wins, 1 loss, mean HP loss 8.06. It is not currently safe enough to keep as a normal candidate.

Interpretation:

- Trace-used potion evaluation is now the correct baseline for combat policy work.
- The HP selector is a real improvement over the selected policy under the corrected potion constraint, but it does not meet the current `<5 mean HP loss` target.
- Remaining loss is concentrated in a small number of high-damage roots, so the next iteration should focus on those fixtures instead of broadening the beam blindly.

### 33. Bounded Principal-Variation Commitment

Change:

- Added `rust_terminal_hp_commit_won_selector_w32_w64_w128_d40`, a diagnostic candidate that follows a Rust principal variation when the selected line is a winning no-potion line.
- Added `rust_terminal_hp_commit_bounded_selector_w32_w64_w128_d40`, a safer variant that only follows a winning no-potion principal variation when:
  - the root has no trace-used potion permission,
  - the line itself uses no potion,
  - the predicted HP loss is at most 31.
- Kept trace-used potion evaluation semantics: each root only allows the potion names the real trace used in that combat window. This mirrors the intended UI boundary where the human chooses whether potions are allowed for a combat.

Why:

- Pure single-action replanning often drifts away from a found winning line.
- Blindly following every found winning line improves many roots but badly regresses some potion-enabled or long-plan roots.
- A bounded commit rule is cheap and deterministic, and it avoids treating potion-enabled roots as safe to commit.

Validation:

```powershell
uv run python -m unittest python.tests.test_self_play python.tests.test_search_smoke -v
uv run python -m sts.self_play eval --trace target\trace-guided\manual01-replayed.jsonl --eval-set full-323 --max-actions 80 --allowed-potions-mode trace_used --candidate rust_terminal_hp_commit_bounded_selector_w32_w64_w128_d40 --output target\search-lab\trace-used-full323-commit-bounded.json
```

Results:

| Candidate | Roots | Wins | Losses | Nonterminal | Mean HP Loss | Median HP Loss | P95 HP Loss | Potion Uses | Mean Seconds / Decision | Mean Seconds / Combat | Mean Search Nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `rust_terminal_hp_selector_w32_w64_w128_d40` | 323 | 323 | 0 | 0 | 6.88 | 4.0 | 36.0 | 36 | 0.0233 | 0.327 | 122880 |
| `rust_terminal_hp_commit_won_selector_w32_w64_w128_d40` | 323 | 323 | 0 | 0 | 6.24 | 3.0 | 39.9 | 23 | 0.0392 | 0.0468 | 18226 |
| `rust_terminal_hp_commit_bounded_selector_w32_w64_w128_d40` | 323 | 323 | 0 | 0 | 5.36 | 2.0 | 31.9 | 36 | 0.0324 | 0.259 | 85117 |

Rejected probes:

| Probe | Result | Decision |
| --- | --- | --- |
| Always width-256 terminal beam, depth 60 | mean HP loss 12.64, 322 wins, 1 loss | too poor globally |
| Always width-256 HP beam, depth 60 | mean HP loss 10.09, 322 wins, 1 loss | too poor globally |
| Always-on selector with width-256 terminal branch | stopped after it ran too long for a practical UI policy | too expensive as an unconditional branch |

Remaining high-loss clusters for the bounded policy:

| Trace Steps | Notes |
| --- | --- |
| 164-172 | Mostly no-potion roots. Bounded commit still loses 49-54 HP on several roots; single-action HP selector is sometimes better, especially 168/171. |
| 94-101 | Cultist/no-potion cluster. Width-256 terminal helps some roots, but is not safe globally. |
| 293-296 | Trace-used Power Potion cluster. Current policy often wins without spending the Power Potion and loses 46-52 HP. |

Interpretation:

- The bounded commit candidate is the best current full-323 trace-used policy by mean HP loss, but it still misses the `<5` target.
- The gap is now small enough that a selector-level improvement could plausibly close it, but broad width increases are not the answer.
- Next iteration should target the listed clusters with cheap conditional rescue rules or better terminal scoring, and should avoid always-on high-width search.
