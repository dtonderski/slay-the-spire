# Awakened One Lethal Sludge Queue

## Witness

`FIDL00441` step 1587 ends with the player at 11 HP facing Awakened One's phase-two Sludge (`move_id=6`, 18 base damage plus 5 Strength). The target's draw pile and `cardRandomRng`-dependent Void placement remain unchanged after the lethal hit.

The simulator calculated Sludge's damage and immediately inserted its Void while the mutable player still held pre-damage HP. Damage application occurs after intent calculation, so checking `player.hp > 0` at insertion time tested stale state and consumed one card RNG draw before inserting a card the target never created.

## Source-backed lifecycle

Awakened One queues Sludge's `DamageAction` before its `MakeTempCardInDrawPileAction`. Target combat termination clears the remaining action queue after lethal damage. If a gameplay revival keeps the player alive, the queued insertion remains eligible to resolve.

## Decision

Gate Sludge's queued Void using the existing damage-survival calculation and the existing `player_can_revive_after_monster_hit` result. Do not special-case the witness seed, alter the insertion index, or burn/restore card RNG. Preserve the existing insertion behavior for surviving and reviving players.

## Surviving-player FIFO

The surviving path applies the full damage pipeline first, then inserts Void
into the resulting draw pile. That lets HP-loss draws that resolve inside
damage (Runic Cube, Centennial Puzzle) take the current top card before
`addToRandomSpot` runs. CommunicationMod leftover piles on FIDL01565,
FIDL01665, FIDL01519, and FIDL01729 match that post-damage pile.

Lethal hits still skip the insert (`player.hp` is already 0). Insertion
bounds stay `cardRandomRng.random(size - 1)`.

FIDL01273 (no HP-loss draws) still first-diverges as an adjacent Void /
Whirlwind+ swap in the leftover draw pile.

## Half-dead PlayTop shuffle

`EmptyDeckShuffleAction` / `PlayTopCardAction` still run while Awakened One is
in first-form half-death (`alive == false`, `mode_shift < 0`). That state is
not `isBattleEnding`. Havoc on an empty draw (FIDL01451) therefore shuffles
discard into draw and force-plays the new top card. The PlayTop no-op that
drops a transient top card applies only after a true combat end (second death),
not during half-death.
