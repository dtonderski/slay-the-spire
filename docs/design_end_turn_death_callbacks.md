# End-turn death callback ordering

FIDL01319 step 628 is a source-backed queue-order regression. Before the
boundary, two Bronze Orbs hold Stasis cards, the player has Combust and
Gremlin Horn, and the draw pile has two cards left. Combust kills both Orbs in
the end-turn power window. The target then publishes each Stasis card and
Gremlin Horn's draw after the visible hand discard, in death order, producing
`Stasis, Horn draw, Stasis, Horn draw` before the normal five-card refill.

The simulator therefore keeps end-turn-power death callbacks as ordered
pairs, settles the hand discard first, and then applies each pair. This does
not alter named RNG: the only boundary RNG calls remain the monster AI roll
and the `shuffleRng` seed used by `Collections.shuffle`. Direct card/monster
turn deaths retain their existing queue path.
