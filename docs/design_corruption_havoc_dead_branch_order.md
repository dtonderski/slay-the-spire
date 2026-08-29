# Corruption Havoc Dead Branch order (FIDL00441)

## STS timing

`Havoc.use` constructs `PlayTopCardAction(getRandomMonster(..., cardRandomRng), exhaust=true)`
during `use()`, consuming the target roll **before** bot actions run. Under
Corruption the skill then self-exhausts via `UseCardAction`, firing Dead Branch
before the forced top card resolves its own `cardRandomRng` (e.g. Sword Boomerang
hits).

Empirical FIDL00441 stream (one living enemy):

`T → DB_havoc → hit × 3 → DB_top` → Dual Wield, Power Through

## Prior sim bug

Corruption used PlayTop-first then self-exhaust, and burned the Havoc target at
PlayTop execute time:

`T → hits → DB_top → DB_havoc`

## Fix

1. When Havoc self-exhausts (Corruption / exhaust keyword): pre-burn the random
   target during queue build; attach it only if the forced top needs `Enemy`.
2. Queue order: settle (exhaust) then PlayTop with `random_living_target: false`.
3. Dead Branch hand-adds insert before a still-queued `PlayTopDrawCard` so DB from
   self-exhaust runs before forced-card RNG.
4. Dark Embrace remains a follow-up behind PlayTop (unit
   `havoc_under_corruption_plays_top_card_before_dark_embrace_from_source_exhaust`).

## Tests

- `corruption_havoc_dead_branch_before_forced_card_rng`
- Existing Havoc suite (including DE under Corruption)

## Empty-draw Corruption

When the draw pile is empty, a *discarded* Havoc still uses PlayTop-first so
the source cannot be the forced refill card. Corruption / exhaust Havoc does
not: `UseCardAction` exhausts the source before `PlayTopCardAction`, so the
source is already out of the refill and Dead Branch must still be
`DB_havoc → DB_top`.

Witness: FIDL01410 step 1279. Empty draw, Corruption Havoc, Giant Head,
Dead Branch. Observed hand after PLAY is `…, Armaments, Sword Boomerang`.
PlayTop-first generated those two DB cards in reverse.

## Residual

FIDL00441 advances past step 1091; later Void insert order still open (~1587).
