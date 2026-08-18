# Mayhem after Brutality start-of-turn draw

## Evidence (FIDL00381 step 883)

End turn into Maw with Mayhem + Brutality. Pre-turn draw top (bottom→top) ends
`… Anger+, Shrug+, … Flex+`. After base draw of five, remaining top is `Shrug+`.

Real next hand includes `Shrug+` and discard gains two `Anger+` (Mayhem played
Anger, which copies itself). Simulator previously ran Mayhem **before** Brutality,
force-played `Shrug+` (block 11), and left Anger in the draw pile.

## Ordering

1. Base hand draw (Evolve draws may remain deferred for Time Warp card-count).
2. Start-of-turn post-draw relics.
3. **Brutality** `atStartOfTurnPostDraw` (lose 1 HP, draw 1) — flush its draws.
4. **Mayhem** force-play top of the post-Brutality draw pile.
5. Flush deferred Evolve draws from the base refill.
6. Flush PlayTop `MakeTempCardInDrawPile` from that Mayhem card (Wild Strike
   Wound, Reckless Charge Dazed). Java queues those behind Evolve's residual
   `DrawCardAction` from the base refill, so `addToRandomSpot` sees the
   post-Evolve pile (FIDL01469: Mayhem PlayTops Wild Strike, Wound is not at
   index 0 of a 15-card remaining pile).

## Implementation

`start_player_turn` in `combat/turn.rs`: move Brutality (+ draw flush) above
`apply_start_of_turn_mayhem`. Keep Evolve residual draws after Mayhem.
Park Mayhem PlayTop random-spot draw inserts until after that Evolve flush.

## Non-goals

- Do not reorder Mayhem ahead of the base five-card refill.
- Do not flush Evolve residual draws before Mayhem PlayTop (Mayhem must still
  play Wild Strike rather than Bludgeon).
- Do not change Havoc Hex mid-PlayTop insert timing.
