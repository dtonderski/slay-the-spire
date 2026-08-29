# Combat Search Benchmark — July 2026

Status: historical experiment conclusion. The raw roots, manifests, and
per-root reports lived under `simulator/combat_research/` through
`6a6f8e989fc47016232eefbea3b2052aab0fd85a` and remain recoverable from Git
history. They are not current-tree fixtures.

## Question

Can a fixed-budget beam planner be improved on lineage-disjoint combat-start
roots without leaking sealed splits or changing the lexicographic objective?

## Frozen protocol

- 225 strict-replay combat-start roots, split by seed lineage.
- Fixed search budget: depth 100, width 300, 100,000 generated transitions,
  10,000 ms timeout.
- Denominator includes losses, nonterminals, illegal actions, errors, and
  timeouts.
- Promotion required more wins without turning an incumbent win into a
  non-win, then higher HP fraction, then potion value, then fewer actions.

## Conclusions

1. The 2026-07-13 complete-turn candidate failed sealed validation (50/56
   wins versus a 52-win gate). Held-out was not opened. The incumbent was
   restored.
2. Weighted terminal sums that trade HP for potions were rejected.
3. Global and frontier `RunState` JSON transposition tables were rejected as
   too slow for the fixed runtime gate.
4. The 2026-07-15 keepers were turn-boundary replanning with a warm suffix,
   lexicographic HP-before-potions comparison, and a budget-neutral
   complete-turn fallback used only when the primary search is nonterminal.
   Development wins moved from 191/202 to 196/202. Sealed holdout survival
   stayed 22/23.
5. Ordinary tests do not depend on these roots. Reproduce a claim by
   checking out the commit above and running the frozen `combat-research`
   evaluator against the historical snapshot.
