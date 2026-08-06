> **Historical note — superseded.** The Letter Opener-specific hidden open
> generations below are not part of the current Discovery lifecycle. The
> observations remain useful historical evidence; see
> [design_discovery_action_update_lifecycle.md](design_discovery_action_update_lifecycle.md)
> for the source-backed model.

# Discovery + Dead Branch open generation (FIDL00394 / FIDL00372)

## Evidence

Hand-played Discovery with Dead Branch reserves the DB card during open-time
hidden `generateCardChoices` burns (`open_discovery_card_reward_for_play`).

Instrumented counter probes after the visible choice generation:

| Trace | Letter Opener | DB after N full gens | Real DB card |
|-------|---------------|----------------------|--------------|
| FIDL00372 | no | N=1 | Twin Strike |
| FIDL00394 | yes | N=3 | Warcry |

## Model

```rust
let (hidden_generations, db_generation) = if relics.contains(LetterOpener) {
    (3, 2)  // three open gens, DB on last
} else {
    (4, 0)  // four open gens, DB after first
};
```

Letter Opener is the distinguishing combat relic on the Warcry witness; treat as
a SuperFastMode DiscoveryAction update-count proxy until a tighter action-manager
link is proven.

## Status

- FIDL00372 remains `complete_pass` with generation 0 / four hidden gens.
- FIDL00394 advances past Discovery CHOOSE (1407 Warcry) to step 1415 Havoc+
  force-play Dead Branch (`Cleave` real vs wrong sim card).
- LO-specific deferred settle-draw offsets can hit Cleave at 1415 but then fail
  Warcry CONFIRM DB at 1419 (`Burning Pact` vs `Dark Embrace`); no single extra
  draw count dual-satisfies both. Residual is post-Discovery `card_random` debt,
  not a further open-generation tweak.
- Permanent corpus stays green with this open-generation branch.
- No permanent trace currently has both Letter Opener and Dead Branch.

## Non-goals

Do not add seed-specific or LO-only deferred draw fudges that only pass one of
{1415, 1419}. Prefer a source-backed residual model that keeps the whole
post-Discovery stream aligned.
