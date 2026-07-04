# Omniscient Combat Search Lab

## Purpose

This lab compares hand-written, non-ML combat search algorithms against fixed
simulator-generated benchmark roots. It is meant to improve the search helper
without contaminating the evaluation with human trace choices.

For the current end-to-end policy-iteration workflow, generated data locations,
and held-out MANUAL01 rules, see `combat_policy_iteration.md` and
`data_manifest.md`.

## Benchmark Discipline

- Roots are generated deterministically from synthetic single- and multi-enemy
  combat states built from the current `OmniCombatEnv` schema and starter
  Ironclad card IDs.
- The root set is split by snapshot hash into `dev` and `eval`.
- Candidate parameters are fixed in code before running `eval`.
- Metrics come from actual simulator rollouts, not imitation of recorded traces.
- Trace-derived roots should be added later when the corpus files are present,
  but traces should remain a separate benchmark family.

Run:

```powershell
uv run maturin develop --release
uv run python -m sts.search_lab --split dev --max-source-depth 2 --max-roots 48 --max-actions 40
uv run python -m sts.search_lab --split eval --max-source-depth 2 --max-roots 200 --max-actions 40
```

## Current Candidates

The synthetic search-lab candidate set includes:

- `exhaustive_basic_d3`: existing heuristic, exhaustive depth 3.
- `exhaustive_tactical_d4`: survival-aware heuristic, exhaustive depth 4.
- `greedy_tactical_d20`: survival-aware heuristic, greedy beam width 1.
- `beam_tactical_w4_d30`: survival-aware heuristic, beam width 4.
- `beam_aggressive_w4_d30`: lethal-biased heuristic, beam width 4.
- `beam_tactical_w8_d40`: survival-aware heuristic, beam width 8.
- `portfolio_rollout_d40`: asks the strongest fixed policies for candidate
  moves, then chooses by the best deterministic rollout outcome across several
  fixed rollout policies.

Current held-out eval result on balanced synthetic roots:

```text
split=eval roots=34 mean_start_hp=51.4
1. portfolio_rollout_d40: win_rate=1.00 score=105279.4 hp=52.8 monster_hp=0.0 nodes=36783.7
2. beam_aggressive_w4_d30: win_rate=0.97 score=98813.5 hp=47.0 monster_hp=0.1 nodes=632.0
3. exhaustive_tactical_d4: win_rate=0.79 score=62940.6 hp=41.7 monster_hp=2.8 nodes=2524.4
4. exhaustive_basic_d3: win_rate=0.74 score=50932.9 hp=39.6 monster_hp=4.2 nodes=692.3
5. beam_tactical_w8_d40: win_rate=0.68 score=38697.6 hp=35.1 monster_hp=5.4 nodes=1536.4
6. beam_tactical_w4_d30: win_rate=0.56 score=14661.2 hp=30.2 monster_hp=6.4 nodes=887.9
7. greedy_tactical_d20: win_rate=0.29 score=-39828.2 hp=15.2 monster_hp=8.8 nodes=308.3
```

## Current Recommendation

`portfolio_rollout_d40` is the strongest current algorithm candidate for
high-quality combat advice. It is much more expensive than
`beam_aggressive_w4_d30`, so UI integration should either label it as a slower
search mode or run it asynchronously. On the smaller `--max-roots 96` held-out
eval slice, the same candidate wins all 21 roots and reaches exact
`mean_final_hp=44.95238095238095`, one total HP below 45. On the intermediate
`--max-roots 128` held-out eval slice, it wins all 27 roots and reaches exact
`mean_final_hp=48.7037037037037`.

Do not treat this as final combat intelligence. The benchmark is still synthetic
and starter-deck focused. The next search-lab expansion should add potion/relic
cases, card-selection states, more enemy behavior diversity, and trace-root
families when the trace corpus is available locally.

## Trace Autopilot Defaults

Trace replay evaluation exposes both historical Python probes and the newer
Rust selector family. The full candidate registry currently includes:

- `tactical_greedy_d40`
- `hp_greedy_d40`
- `trace_probe_d40`
- `trace_probe_potion_rescue_d40`
- `trace_probe_aggressive_rescue_d40`
- `trace_probe_no_potions_d40`
- `rust_greedy_tactical_d40`
- `rust_beam_tactical_w16_d40`
- `rust_beam_terminal_w16_d40`
- `rust_beam_terminal_w32_d40`
- `rust_beam_terminal_w128_d40`
- `rust_beam_terminal_w128_no_power_d40`
- `rust_terminal_rescue_w32_w128_no_power_d40`
- `rust_terminal_rescue_keyed_w32_w128_no_power_d40`
- `rust_terminal_win_hp_bounded_w32_d40`
- `rust_terminal_win_hp_selector_w32_w128_no_power_d40`
- `rust_terminal_hp_selector_w32_w64_w128_d40`
- `rust_terminal_hp_commit_won_selector_w32_w64_w128_d40`
- `rust_terminal_hp_commit_bounded_selector_w32_w64_w128_d40`
- `rust_terminal_hp_commit_safe_selector_w32_w64_d40`
- `rust_terminal_hp_commit_safe_boss_selector_w32_w64_w128_d40`
- `rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40`
- `rust_terminal_rollout_selector_w32_w128_no_power_d40`
- `rust_terminal_portfolio_d40`

These are intentionally separate from the historical synthetic benchmark
defaults. Expensive candidates such as `portfolio_rollout_d40` are still useful
diagnostics, but they should be run explicitly rather than as part of every
trace-root iteration loop.

The newer `iterate-combat-policy` command uses an even tighter all-Rust default
candidate set for full train/dev plus held-out all-state reports. See
`combat_policy_iteration.md` for the current canonical run and candidate list.

## Comparison Warnings

- `trace_used` is the canonical MANUAL01 potion mode. Unrestricted potion probes
  can find lines that are illegal for the held-out comparison.
- `combat_start` and `all_decision_states` answer different questions. Use both
  when promoting an autopilot candidate.
- "Best ranked" is not the same as "lowest mean HP loss"; terminal outcomes and
  nonterminal episodes are part of the ordering.
- Any exact-action invariant failure or strict replay failure is a simulator
  bug first and a policy result never.
