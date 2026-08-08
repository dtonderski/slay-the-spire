# Empty-draw dual-Havoc settle-first (FIDL00238)

## Observation (step 953)

- Pre: block 3, draw empty, discard `[Havoc]`, hand includes `Havoc+`, Feel No Pain 3.
- Post: block **9**, both `Havoc` and `Havoc+` in **exhaust**.

## Rule

Empty-draw Havoc normally queues **PlayTop before source settle** so the refill
cannot include the source (Sever Soul / mixed discard / FIDL00238 step 873).

**Exception:** when the discard pile is non-empty and **only** contains
Havoc/Havoc+, settle the source into discard first. Nested force-play then
chain-exhausts both Havocs (dual FNP). Dark Embrace still forces PlayTop-first.

## Tests

- `havoc_empty_draw_dual_havoc_chain_exhausts_both` — both exhaust, block +6 FNP
- `havoc_empty_draw_mixed_discard_play_top_first_keeps_source_out_of_reshuffle`
- `havoc_force_sever_soul_does_not_exhaust_havoc_source`

## Status

FIDL00238 promoted complete_pass. Mixed-discard PlayTop-first retained.
