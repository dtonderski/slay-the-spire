> **Historical note — superseded.** The Dark Embrace-specific Discovery RNG
> settle draw below depended on the abandoned cross-command residual model.
> See [design_discovery_action_update_lifecycle.md](design_discovery_action_update_lifecycle.md)
> for the current source-backed lifecycle.

# Discovery pick settle draw with Dark Embrace (FIDL00393)

## Evidence

Hand-played Discovery under SuperFastMode burns post-pick residual
`generateCardChoices` updates before the next combat command. With
`HAND_PLAYED_DISCOVERY_PICKED_SCREEN_SETTLE_DRAWS = 0`, FIDL00393's next-turn
Magnetism rolls Finesse; real has Flash of Steel. Probe: Flash is **2** pool
singles past Finesse at that Magnetism, and **+1** settle draw at Discovery pick
lands Flash while preserving permanent FIDL00226 (Magnetism without Dark Embrace).

FIDL00226 also has Magnetism at its Discovery pick; branching on Magnetism power
breaks Dramatic Entrance. Branching on **Dark Embrace > 0** keeps 226 green.

## Model

```rust
let settle = if combat.player.powers.dark_embrace > 0 {
    HAND_PLAYED_DISCOVERY_PICKED_SCREEN_SETTLE_DRAWS + 1
} else {
    HAND_PLAYED_DISCOVERY_PICKED_SCREEN_SETTLE_DRAWS
};
```

Dark Embrace's onExhaust `DrawCardAction` is bot-queued in the same UseCardAction
window as Discovery source exhaust. Extra SuperFastMode DiscoveryAction update
count in that window is the working proxy.

## Status

- FIDL00393 advances past END 372 (Flash of Steel) to END 373 (Magnetism card
  real vs Secret Weapon sim). Residual remains later in the Magnetism stream.
- Permanent corpus green with this branch.
- FIDL00411 / FIDL00413 have Magnetism but no Dark Embrace; unaffected.

## Non-goals

Do not move hand-discovery deferred END burns before EndTurn/Magnetism without
retuning the full stage machine (breaks 226 Magnetism ordering).
