# Run-State Phase Validation

`RunState` is restored from snapshots and raw debugging JSON, so its `phase`
must not make missing screen state look like a valid transition target. Phase
validation should reject missing authoritative owners without rejecting the
overlays that the simulator intentionally preserves.

## Required owners

- `Combat` requires `combat`; combat state is forbidden in every other phase.
- `Reward` requires `reward`. A card grid may coexist with the reward, but is
  not a substitute for the reward screen.
- `Event` requires `event`. Event card grids retain their event screen.
- `Shop` requires `shop`, whether the merchant overlay is open or closed.
- `Treasure` requires `treasure_room`, except for the boss relic chest flow,
  whose room identity is `Boss` and whose chest state is represented by
  `boss_chest_opened` and the reward transition.

`Idle` does not yet require a map because explicit combat fixtures can settle
to idle without owning a run map. `Rest` has no separate screen object; its
phase and `rest_room_complete` flag are currently the authoritative state.
`Complete` is also a generic terminal state: a real run may retain its terminal
Spire Heart screen, while explicit environments need no screen. Those
representations should be strengthened only alongside an explicit state model
or fixture migration.

## Overlay policy

Owner state may remain populated outside its primary phase when it is needed
as an explicit continuation. Examples are event-to-reward, shop-to-reward,
treasure-to-reward, and rest-to-reward flows. Validation therefore enforces
required owners, not blanket field clearing.

Malformed imported state must fail with `InvalidState`; validation must never
create a missing screen, choose a fallback, or normalize the phase.
