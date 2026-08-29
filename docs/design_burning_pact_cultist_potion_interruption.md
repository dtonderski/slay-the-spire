# Burning Pact and Cultist Potion interruption

## Decision

When Cultist Potion is used while Burning Pact is waiting for its exhaust
selection, the selected card is not immediately exhausted or exposed in a
visible pile. The simulator keeps that authoritative card instance pending
until end-player-turn cleanup. Visible hand cards are discarded first; the
pending card is then appended to discard so it participates in the same
discard-to-draw shuffle with the target ordering.

This is limited to the active Burning Pact exhaust-selection state. It does
not infer a card from an observation, repair a missing pile, or change the
normal Burning Pact path.

## Evidence

The real-game trace
`random-fidelity-95f6caf61e3c923f.jsonl` shows the selected Strike UUID in
`screen_state.selected`, absent from hand, draw, discard, and exhaust after
`CONFIRM`, and present in the draw pile after the next `END`. Running the
authoritative simulator with the pending card appended before visible-hand
cleanup placed it at the wrong post-shuffle position. Appending it after that
cleanup reproduces the complete observed hand and draw projections at step
98 with no simulator-to-observation diff.

The persistent trace remains the primary regression oracle; the small core
test protects the ordering boundary without using observed state as input.
