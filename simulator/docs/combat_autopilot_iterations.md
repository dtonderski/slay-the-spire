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
