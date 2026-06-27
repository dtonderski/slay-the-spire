# Omniscient Combat Search Lab

## Purpose

This lab compares hand-written, non-ML combat search algorithms against fixed
simulator-generated benchmark roots. It is meant to improve the search helper
without contaminating the evaluation with human trace choices.

## Benchmark Discipline

- Roots are generated deterministically from synthetic combat states built from
  the current `OmniCombatEnv` schema and starter Ironclad card IDs.
- The root set is split by snapshot hash into `dev` and `eval`.
- Candidate parameters are fixed in code before running `eval`.
- Metrics come from actual simulator rollouts, not imitation of recorded traces.
- Trace-derived roots should be added later when the corpus files are present,
  but traces should remain a separate benchmark family.

Run:

```powershell
$env:PYTHONPATH = "$PWD\python"
py -3.14 -m sts.search_lab --split dev --max-source-depth 2 --max-roots 48 --max-actions 40
py -3.14 -m sts.search_lab --split eval --max-source-depth 2 --max-roots 48 --max-actions 40
```

## Current Candidates

The current candidate set includes:

- `exhaustive_basic_d3`: existing heuristic, exhaustive depth 3.
- `exhaustive_tactical_d4`: survival-aware heuristic, exhaustive depth 4.
- `greedy_tactical_d20`: survival-aware heuristic, greedy beam width 1.
- `beam_tactical_w4_d30`: survival-aware heuristic, beam width 4.
- `beam_aggressive_w4_d30`: lethal-biased heuristic, beam width 4.
- `beam_tactical_w8_d40`: survival-aware heuristic, beam width 8.

Current held-out eval result on synthetic roots:

```text
split=eval roots=9
1. beam_aggressive_w4_d30: win_rate=1.00 score=102666.7 hp=26.7 monster_hp=0.0 nodes=516.1
2. exhaustive_tactical_d4: win_rate=0.89 score=80451.1 hp=27.0 monster_hp=1.3 nodes=1454.8
3. exhaustive_basic_d3: win_rate=0.78 score=57504.4 hp=20.1 monster_hp=3.1 nodes=485.2
4. beam_tactical_w8_d40: win_rate=0.56 score=12153.3 hp=11.7 monster_hp=6.2 nodes=1397.2
5. beam_tactical_w4_d30: win_rate=0.56 score=12053.3 hp=10.7 monster_hp=6.2 nodes=806.4
6. greedy_tactical_d20: win_rate=0.44 score=-10324.4 hp=9.2 monster_hp=6.8 nodes=267.4
```

## Current Recommendation

`beam_aggressive_w4_d30` is the best current algorithm candidate for combat
advice. It should be the next candidate to try behind the UI Search button after
we decide the UI/API shape for choosing algorithms.

Do not treat this as final combat intelligence. The benchmark is still narrow:
single-enemy synthetic Ironclad fights only. The next search-lab expansion should
add multi-enemy roots, potion/relic cases, card-selection states, and trace-root
families when the trace corpus is available locally.
