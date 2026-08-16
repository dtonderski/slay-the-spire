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

The same empty-hand check applies after a confirmed Forethought / Forethought+
selection that leaves no remaining cards. Dark Embrace or another on-exhaust
refill that already restored the hand suppresses the relic, matching the
existing `UnceasingTopDraw` emptiness guard.

## Evidence

- FIDL01739 step 624: hand is `Bludgeon, Forethought` with Unceasing Top.
  After playing Forethought, Bludgeon is on the bottom of the draw pile and
  Wound (previous top) is in hand.
- Existing unit coverage keeps auto-place destination and zero-cost override
  unchanged when Unceasing Top is absent.

## Non-goals

- Do not change skipped PutOnDeck retrieval.
- Do not invent a seed-specific Forethought cost or destination.
- Do not apply Unceasing Top when the hand is still occupied after settlement.
