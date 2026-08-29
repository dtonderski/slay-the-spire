# Relic canSpawn retry pop direction

## STS (`AbstractDungeon`, javap)

- `returnRandomRelicKey` pops **front** (`remove(0)`). On `!canSpawn()`, calls
  `returnEndRandomRelicKey` (switch to **end**).
- `returnEndRandomRelicKey` pops **end** (`remove(size-1)`). On `!canSpawn()`,
  calls **itself** (stay on **end**). Boss tier always pops front.

## Simulator

`RelicPoolState::return_random_relic_from` on `!relic_can_spawn` always retries
with `from_front = false` (end). Do **not** preserve the original direction:
front-only retries desync Scrap Ooze / elite / shop when Bottled* are skipped
(FIDL00438 elite was Paper Phrog vs Ornamental Fan under a mistaken preserve).

## Regression

Permanent `random-fidelity-fc48f9b15e358fb6` Scrap Ooze → Ornamental Fan.
