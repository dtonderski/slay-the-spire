# Time Warp forced-end queue multiplicity

A 12th card that closes a hand/exhaust selection can leave CommunicationMod at a
pre-end-turn frame while `TimeWarpPower.onAfterUseCard` has already armed the
forced end. The following explicit `END` is then processed after that queued
end-turn request. In the source action manager, the resulting monster queue can
contain two `MonsterQueueItem`s before either item's `RollMoveAction` runs.
Each item captures the same current intent; each later roll advances the move
history. This is observable generically in FIDL00372 (two Head Slam damage
packets, `times_damaged +2`, move history ending `last=2, second_last=4`) and
FIDL00275 (two Haste rolls, move history ending `last=2, second_last=5`).

The simulator therefore keeps a transient duplicate-queue marker on the
source-backed hand/exhaust selection lag state. End-turn settlement consumes the
marker and executes the captured second monster queue item, rather than
suppressing the explicit `END`. Time Warp's +2 Strength is applied before the
queue, while queued attack damage uses the pre-Strength-action damage value;
this matches the source `DamageInfo` objects captured by `Time Eater.takeTurn`.
The marker is not serialized into observations and is never selected by seed or
trace identity.
