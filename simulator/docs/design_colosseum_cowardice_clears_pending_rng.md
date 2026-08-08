# Colosseum COWARDICE clears parked combat RNG (FIDL00438)

## Bug

After the first Colosseum fight, victory returns to the event with
`pending_event_combat_rng` set so a second bout can continue the shuffle /
monster-HP streams (`enter_event_combat` consumes that park and burns one
`randomLong` on each).

Choosing **COWARDICE** (escape) left that park in place. A later event combat
(Mind Bloom “I am War”) then incorrectly reused the Colosseum first-fight
shuffle stream instead of `seed_for_floor(event_rng_seed, floor)`, desyncing
opening hand/draw (FIDL00438 step 830).

## Fix

On Colosseum stage-2 choice 0 (COWARDICE), clear `pending_event_combat_rng`
before leaving the event.

## Non-goals

- Do not change the second-fight (VICTORY) path that legitimately consumes the
  park.
