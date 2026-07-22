# Run screen ownership validation

`RunState` contains both the active run phase and optional state for screens that
can temporarily remain underneath an overlay. Validation must distinguish those
typed continuations from orphaned imported state.

## Ownership rules

- `combat` and `reward` are exclusive to `Combat` and `Reward`, respectively.
- `event`, `shop`, completed-rest, and treasure-room state may remain underneath
  `Reward` only when the reward's typed continuation names that owner.
- Outside a reward overlay, `event`, `shop`, completed-rest, and treasure-room
  state belong only to `Event`, `Shop`, `Rest`, and `Treasure`, respectively.
  The completed Spire Heart event is the sole terminal event retained in
  `Complete`, with its exact terminal stage and empty choice set validated.
- An open merchant belongs to an active shop or a shop reward continuation.
- Boss-chest state belongs to a boss-room `Reward` or `Treasure` phase.
- Card-grid purposes name their owner. Rest, shop, and event/Neow grids require
  their corresponding phase and screen. Boss-relic grids belong either to Neow
  (`Event`) or an opened boss chest during boss-relic resolution (`Treasure`).
  Return-to-event grids must name the retained event. Bottle grids may be
  opened by event, reward, or shop relic acquisition.

The validator rejects contradictory combinations. It does not infer a missing
owner, select a plausible continuation, or repair imported state. Transitions
must clear state when its owner ends; in particular, closing a normal treasure
reward clears the retained treasure-room state before returning to the map.

This changes validation and state cleanup only. It does not change snapshot
shape or silently migrate malformed raw state.
