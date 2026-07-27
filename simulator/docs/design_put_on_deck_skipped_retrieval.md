# Put-on-deck skipped retrieval frame

## Source behavior

Vanilla `PutOnDeckAction` opens `HandCardSelectScreen` when it needs a card
from the hand. On confirmation, its later update iterates
`handCardSelectScreen.selectedCards` and calls `hand.moveToDeck(card, false)`.
The action calls `tickDuration()` when it first opens the screen. If the action
has already completed before the screen is confirmed, `GameActionManager`
skips that later update. The selected card remains owned by the selection
screen object and is absent from the serialized hand, draw, discard, exhaust,
and limbo piles after the screen closes.

The target JAR evidence is the decompiled `PutOnDeckAction.update()`,
`CardGroup.moveToDeck()`, and `GameActionManager.update()` methods from
`desktop-1.0.jar`. CommunicationMod serializes the four piles and limbo from
the player directly, while serializing selected hand cards separately under
`screen_state.selected`.

## Verifier contract

The normal core transition keeps the intended selected-card draw destination
authoritative. The real-game skipped-retrieval result is a separate,
source-backed transition candidate. It is eligible only for a typed Warcry,
Thinking Ahead, or base Forethought hand-select confirmation, only when the
selected card is in the core draw pile, and only when the stable target frame
matches the candidate in every other compared field. A generic pile mismatch
does not use this candidate.

Once that exact target transition is verified, later commands continue from
the skipped-retrieval candidate. The candidate is derived only by removing the
simulator-selected card from the normal core result; no observed card identity,
pile ordering, RNG state, or other gameplay field is copied into simulation.
This is necessary because the target card remains owned by the closed
selection-screen object after the action manager skips retrieval.

The limbo card is re-introduced through the end-turn discard path
(`pending_hidden_hand_card_until_end_turn` → discard) on the next END that
discards a multi-card hand. Injecting it on empty/single-card ENDs that
reshuffle discard into the next refill desyncs draw/hand order from the target
(see `random-fidelity-ae18829cad583a71` / `b788a4e142c8fc26`). Forethought
still waits one full refill before that settlement window. The master deck
remains unchanged.

The supported single-card sources are Warcry, Thinking Ahead, and base
Forethought. Forethought+ can select multiple cards and requires a separate
projection because its selected-card multiplicity and destination ordering
are different.
