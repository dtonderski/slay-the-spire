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
