# Observation-independent treasure binding

## Problem

The seed-start verifier previously inspected the observed post-screen after
`CHEST` / `CHOOSE 0`. An observed `BOSS_REWARD` caused it to construct a boss
relic reward directly; every other observed screen caused it to construct an
ordinary chest reward. The observation therefore selected and mutated the
authoritative simulator transition.

Treasure `PROCEED` also treated an observed `NONE` boss-room frame as verified
without reconciling the simulator-owned next-act map, and could use an observed
`MAP` screen to select the verifier's next semantic phase.

## Binding rule

In `SeedStartPhase::Treasure`, `CHOOSE 0` binds to exactly one typed core action:
`RunAction::OpenChest`. Core state determines whether the room is a boss room
and constructs either the boss-relic or ordinary chest reward. The verifier
then validates the resulting phase, reward ownership, boss-chest marker, and
boss-choice shape before choosing a projector. Any inconsistent combination
fails at `invalid_treasure_destination`.

`PROCEED` similarly binds to `RunAction::Proceed`. It must produce an idle run
in a new act, after which the verifier projects the deterministic map and enters
its map phase. An observed screen is comparison input only; `NONE` is no longer
accepted as a complete map verification.

## Regression contract

A forged-trace test changes an ordinary chest's observed post-screen from
`COMBAT_REWARD` to `BOSS_REWARD`. Verification must retain the core-owned
`open treasure chest` transition and report the screen mismatch. Permanent
corpus replay covers both ordinary and boss chest openings and next-act map
transitions.
