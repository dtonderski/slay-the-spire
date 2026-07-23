# Potion Discovery post-selection settlement

`DiscoveryAction.update()` generates a fresh three-card choice set before it
processes the selected card. The target game's CommunicationMod can accept an
`END` command while that action is still settling, so the card RNG continues to
advance across the following player-turn boundary. The simulator records this
pending lifecycle in `CombatState` instead of deriving it from an observed
post-state.

The permanent trace `random-fidelity-a741796d1b33e9a3` is the regression oracle:
after the first Sword Boomerang, the pending Power Potion action consumes twelve
typed choice generations and one final typed draw before the next Sword. The
settlement is applied on the second subsequent `END`, matching the action queue
timing exposed by CommunicationMod.
