# Act transition relic pool reinitialization

## Evidence

Target `AbstractDungeon.initializeRelicList` populate/shuffle/strip is the
**run-start** sequence:

1. Clear common / uncommon / rare / shop / boss pools.
2. `RelicLibrary.populateRelicPool` for each tier.
3. When `floorNum >= 1`, queue every owned player relic id into
   `relicsToRemoveOnStart`.
4. Shuffle each pool with `Collections.shuffle(..., new Random(relicRng.randomLong()))`
   (five `randomLong` draws).
5. Remove every `relicsToRemoveOnStart` id from all pools.

Shop inventory uses `returnRandomRelicEnd` (pop pool tail) after
`merchantRng` tier rolls. Those leftover tails persist into later acts.

## Helper

`RunState::reinitialize_ironclad_relic_pools_for_new_act` implements the
run-start shuffle + strip. Unit-tested. Do not invoke it from act entry.

## Wiring status

`initializeRelicList`'s populate/shuffle/strip sequence is the run-start
helper (`ensure_ironclad_relic_pools` / `reinitialize_ironclad_relic_pools_for_new_act`).
It is **not** called from `enter_next_act_map`.

Act-2 traces pin leftover act-1 order, not a second shuffle on the live
`relicRng` counter:

- FIDL01244 floor-26 uncommon chest is `Darkstone Periapt`, the untouched
  front of the act-1 shuffled uncommon pool. Continued-counter reinit offers
  `Blue Candle`.
- FIDL01245 floor-19 shop is `Mercury Hourglass`, `Singing Bowl`,
  `Clockwork Souvenir` — the depleted uncommon/shop tails after the act-1
  shop. Continued-counter reinit offers `Question Card` / `Letter Opener` /
  `Membership Card` at the same merchant prices.

Resetting `relicRng` and reshuffling would put Darkstone first again but
would also re-offer the act-1 shop-tail relic (`Cauldron`), which the act-2
shop does not. Persist the leftover lists and leave `relic_rng_counter`
unchanged at act entry.

## Related fix landed

`return_random_relic_from` canSpawn retry always continues from the **end**
(`from_front=false`). STS `returnRandomRelicKey` on `!canSpawn` calls
`returnEndRandomRelicKey`; `returnEndRandomRelicKey` on `!canSpawn` calls itself
(javap `abstractdungeon.full.javap`). Do not preserve front-only retries.
