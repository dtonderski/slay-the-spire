# Omniscient Combat Search Lab

## Purpose

This lab compares hand-written, non-ML combat search algorithms against fixed
simulator-generated benchmark roots. It is meant to improve the search helper
without contaminating the evaluation with human trace choices.

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

Current held-out eval result on balanced synthetic roots:

```text
split=eval roots=21
1. beam_aggressive_w4_d30: win_rate=0.95 score=94364.8 hp=38.9 monster_hp=0.1 nodes=588.4
2. exhaustive_tactical_d4: win_rate=0.76 score=55793.3 hp=34.8 monster_hp=3.4 nodes=2449.3
3. exhaustive_basic_d3: win_rate=0.71 score=46141.9 hp=33.8 monster_hp=4.6 nodes=665.4
4. beam_tactical_w8_d40: win_rate=0.67 score=36259.0 hp=30.5 monster_hp=6.1 nodes=1423.5
5. beam_tactical_w4_d30: win_rate=0.57 score=16858.1 hp=27.1 monster_hp=6.9 nodes=820.7
6. greedy_tactical_d20: win_rate=0.29 score=-41721.0 hp=13.2 monster_hp=9.1 nodes=280.5
```

## Current Recommendation

`beam_aggressive_w4_d30` is the best current algorithm candidate for combat
advice. It should be the next candidate to try behind the UI Search button after
we decide the UI/API shape for choosing algorithms.

Do not treat this as final combat intelligence. The benchmark is still synthetic
and starter-deck focused. The next search-lab expansion should add potion/relic
cases, card-selection states, more enemy behavior diversity, and trace-root
families when the trace corpus is available locally.
