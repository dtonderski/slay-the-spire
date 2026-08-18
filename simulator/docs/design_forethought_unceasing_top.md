# Forethought Unceasing Top after empty-hand settlement

## Source behavior

Unceasing Top draws one card when the published hand becomes empty mid-turn.
`MoveCard` already queues that draw after a card leaves the hand. Base
Forethought does not use that path when it auto-places the only other hand
card: `ForethoughtAutoMove` puts the other card on the bottom of the draw pile
and then settles Forethought itself via delayed source discard/exhaust.

After that settlement the published hand is empty, so Unceasing Top must still
draw the current top of the draw pile. The selected card is already on the
bottom, so the draw is the previous top, not the Forethought target.

Warcry singleton auto-place is the same delayed-source family: `getRandomCard`
plus `moveToDeck` empties the published hand (Warcry is in limbo), then source
exhaust. Unceasing Top then draws that top card. Without the relic draw, the
skip-auto-place candidate matches the observed one-card hand and replay drops
the `getRandomCard` roll.

The same empty-hand check applies after a confirmed Forethought / Forethought+
selection that leaves no remaining cards. Dark Embrace or another on-exhaust
refill that already restored the hand suppresses the relic, matching the
existing `UnceasingTopDraw` emptiness guard.

## Evidence

- FIDL01739 step 624: hand is `Bludgeon, Forethought` with Unceasing Top.
  After playing Forethought, Bludgeon is on the bottom of the draw pile and
  Wound (previous top) is in hand.
- FIDL01461 step 1425: last-card Warcry with Unceasing Top already drawn that
  Warcry this turn. Auto-place puts Clothesline+ on top; Top redraws it. The
  burned `cardRandomRng` is required so later Madness zeros Strike, not Entrench.
- Existing unit coverage keeps auto-place destination and zero-cost override
  unchanged when Unceasing Top is absent.

## Non-goals

- Do not change skipped PutOnDeck retrieval.
- Do not invent a seed-specific Forethought cost or destination.
- Do not apply Unceasing Top when the hand is still occupied after settlement.
