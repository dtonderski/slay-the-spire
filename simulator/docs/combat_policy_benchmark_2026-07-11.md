# Combat Policy Benchmark — 2026-07-11

The corpus-wide train/validation gate is intentionally excluded from ordinary
`cargo test` runs because it performs full combat searches across collected
traces. Run it explicitly when evaluating combat-policy changes:

```powershell
cargo test -p sts_live collected_trace_benchmark_reports_train_and_validation_reward -- --ignored --nocapture
```

## Question

Evaluate the combat-policy changes that replace the transient 5,000-point
potion-use penalty with a persistent eight-HP-equivalent inventory value and
only report final HP for terminal search lines.

## Split and provenance

- Train: four simulator-generated traces (`TRAIN01` through `TRAIN04`), grouped
  by seed; 16 combat-start roots and 162 all-decision roots.
- Development: two disjoint simulator-generated traces (`DEV01`, `DEV02`),
  grouped by seed; 10 combat-start roots and 91 all-decision roots.
- Real-data validation: the strict, unmodified prefix of retained MANUAL01.
  The prefix self-verifies for 24 steps with zero anchors and zero
  restorations, but stops at an unsupported shop `PROCEED` mapping. It contains
  one combat and nine decision roots, with no potion-action roots.

Generated reports are under
`simulator/target/combat-policy-benchmark/iteration-v1/` and are intentionally
not committed.

## Candidate selection

Four predeclared candidates were compared without looking at the real-data
scoreboard. `rust_beam_terminal_w16_d40` ranked first on development
combat-start roots: 10/10 wins, zero nonterminal episodes, 1.5 mean HP loss,
and three potion uses. It was then used for the old-vs-new penalty A/B.

## A/B on identical development roots

| Scope | Roots | Old penalty HP loss | Persistent value HP loss | Old / new potion uses | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| Combat start | 10 | 1.200 | 1.500 | 3 / 3 | New scoring regressed by 0.300 HP/root. |
| All decisions, capped | 64 | 0.250 | 0.141 | 6 / 6 | New scoring improved by 0.109 HP/root. |

All A/B episodes won and terminated. The change did not increase potion-use
count on this sample; it changed beam ordering around potion lines.

## Real-data scoreboard

On the nine strict MANUAL01 decision roots, both policies won 9/9 with mean HP
loss -0.222 and zero delta versus the recorded line. These roots contain no
legal potion actions, so they do not validate the potion behavior. Only one
independent real combat is represented, so no confidence claim is warranted.

## Decision

Treat the result as diagnostic, not a validated promotion. The persistent
potion valuation fixes a scoring inconsistency and the all-decision development
metric improved slightly, but combat-start performance regressed slightly and
the available strict real prefix has no potion coverage. Keep the regression
tests, but require a larger trace-grouped real validation corpus containing hard
fights and potion opportunities before claiming policy improvement.

