# Act transition relic pool reinitialization

## Evidence

Target `AbstractDungeon.initializeRelicList` (bytecode) runs during each
act/dungeon setup:

1. Clear common / uncommon / rare / shop / boss pools.
2. `RelicLibrary.populateRelicPool` for each tier.
3. When `floorNum >= 1`, queue every owned player relic id into
   `relicsToRemoveOnStart`.
4. Shuffle each pool with `Collections.shuffle(..., new Random(relicRng.randomLong()))`
   (five `randomLong` draws).
5. Remove every `relicsToRemoveOnStart` id from all pools.

Shop inventory uses `returnRandomRelicEnd` (pop pool tail) after
`merchantRng` tier rolls. Without per-act reinit, act 2/3 shops keep the
depleted act-1 pool order while the real game reshuffles.

## Helper

`RunState::reinitialize_ironclad_relic_pools_for_new_act` implements the
shuffle + strip. Unit-tested.

## Wiring status

Called from `enter_next_act_map`. Enabling it on FIDL00241 moves the first
boundary earlier to act-2 treasure step 551 (`Singing Bowl` real vs `Kunai`
sim), which implies `relic_rng` counter (or owned-set) at the act boundary is
not yet aligned even though act-1 relic offers matched under the depleted pool.
Do not “fix” FIDL00241 by observation rebinding or by skipping this reinit.

## Related fix landed

`return_random_relic_from` canSpawn retry always continues from the **end**
(`from_front=false`). STS `returnRandomRelicKey` on `!canSpawn` calls
`returnEndRandomRelicKey`; `returnEndRandomRelicKey` on `!canSpawn` calls itself
(javap `abstractdungeon.full.javap`). Do not preserve front-only retries.
