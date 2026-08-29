# Nested Havoc PlayTop exhaust order (FIDL00394)

## STS order

`AbstractPlayer.useCard`:

1. `card.use()` — Havoc `addToBot(PlayTopCardAction(randomMonster, exhaust=true))`
2. `addToBottom(UseCardAction)` — settles the played card (`exhaustOnUseOnce` from a parent PlayTop)

A **top-played Havoc** therefore runs its nested PlayTop **before** it force-exhausts.

## Bug

`play_top_draw_card_queue` re-inserted the force-exhaust `MoveCard` at the
hand-play builder settle index (before nested `PlayTopDrawCard`). `card_random`
became `T,DB,T,DB` instead of `T,T,DB,DB`.

FIDL00394 dual Havoc → Doubt + Dead Branch expected Fiend Fire then Sword Boomerang.

## Fix

When the top-draw use queue contains `PlayTopDrawCard`, append source settlement
at the **end** of the queue. Other top-draw cards keep the relative MoveCard index.

PlayTop then parks the forced card as `ResolveTopDrawCard` (the card queue).
Dead Branch `MakeTempCardInHand` is still an action-queue `addToBot` and must
insert *before* that parked resolve so the nested Havoc's Dead Branch lands
before the inner card's (FIDL01582 Havoc+ → Havoc → Strike: Feel No Pain then
Sword Boomerang). Dark Embrace draws already used this insert.

## Tests

- `dual_havoc_doubt_dead_branch_probe`
- Existing Havoc unit suite remains green

## Residual

FIDL00394 continues at step 1407 (Discovery + Dead Branch Warcry vs Spot Weakness).
Do not thrash `DISCOVERY_ACTION_HIDDEN_GENERATIONS` DB reserve index — generation==2
fixes 394 but regresses the FIDL00372 witness.
