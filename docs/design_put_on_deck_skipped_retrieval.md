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
Thinking Ahead, or base Forethought hand-select confirmation, and only when
the stable target frame matches the candidate in every other compared field.
A generic pile mismatch does not use this candidate.

Once that exact target transition is verified, later commands continue from
the skipped-retrieval candidate. The candidate is rebuilt only from the
pre-CONFIRM simulator state via
`confirm_hand_select_skipped_put_on_deck_retrieval`: remove the selected card
without putting it on the draw pile, then settle the source (exhaust/discard
and on-exhaust effects). No observed card identity, pile ordering, RNG state,
or other gameplay field is copied into simulation.

Rebuilding from pre-CONFIRM is required when Dark Embrace (or another
on-exhaust draw) is active. The normal core path puts the selected card on
top and then Dark Embrace draws it into hand; stripping that card from the
post-CONFIRM result would leave the wrong card undrawn. Under skipped
retrieval the card never reached the draw pile, so Dark Embrace draws the
previous top instead (e.g. Dual Wield still in draw → drawn into hand while
Defend+ stays in selection-screen limbo).

The limbo card is re-introduced through the end-turn discard path
(`pending_hidden_hand_card_until_end_turn` → discard):

- On the first END after skipped retrieval with a non-empty hand
  (`hand_len >= 1`), including single-card hands
  (`random-fidelity-e3f0cee2cea07d40`, `d5c980b70d7d6924`,
  `b4f5134b0a0ebd6e`).
- Empty-hand ENDs never inject: they reshuffle discard into the next refill
  and would desync draw/hand order (`ae18829cad583a71` / `b788a4e142c8fc26`
  step 472).
- After an empty-hand miss, require a multi-card hand (`hand_len >= 2`) before
  injecting; a later single-card END still holds (`ae18829` step 477 → 481).

Warcry, Thinking Ahead, and base Forethought share this settlement window. The
master deck remains unchanged.

If combat ends on that first END before hand discard (end-of-turn powers such
as Combust kill the last enemy), the limbo card never enters discard and is
not carried as a cross-combat residual discard instance. Master deck still
supplies the live copy next combat. Injecting a residual here falsely
lengthens the next combat's discard (`null != "Dropkick"` on
`random-fidelity-f3c0d2bea83d9313`).

A mid-turn lethal PLAY after skipped Warcry/Thinking Ahead/Forethought is
different: no END ran, so `selectedCards` stays on the singleton screen. The
next combat shuffles in the live master-deck copy, and a later END can publish
the leftover instance beside it with the same observed UUID (FIDL01365 Strike,
FIDL01467 Thunderclap, FIDL01795 Headbutt). The verifier remints a transient
instance id so both copies stay unique. An empty-hand END in the same combat
does not consume that window.

Burning Pact deferred exhaust selection uses the same residual publication
after a mid-turn lethal blow (`random-fidelity-6e6f4f8c`, FIDL01323).

The supported single-card sources are Warcry, Thinking Ahead, and base
Forethought. Forethought+ can select multiple cards and requires a separate
projection because its selected-card multiplicity and destination ordering
are different.

## Auto path when `hand.size() <= amount`

Vanilla `PutOnDeckAction.update()` opens `HandCardSelectScreen` only when
`hand.size() > amount`. Otherwise it places every hand card via
`getRandomCard(cardRandomRng)` (including `random(0)` for a singleton) with no
player decision. Warcry is in limbo during that check, so a lone post-draw
card auto-completes: no HAND_SELECT / CHOOSE / CONFIRM, and END is immediately
legal (`random-fidelity-58c2f0f27ef22764` step 468–469). The core
`AwaitHandSelect` path for `WarcryPutOnDraw` mirrors that size gate using the
non-source hand (limbo stand-in) and advances `card_random_rng` on auto-place.

If that auto-place update never runs — the same SuperFastMode skipped-retrieval
family as a confirmed PutOnDeckAction — Warcry still exhausts and the drawn card
stays in hand. `getRandomCard` is not burned. The ordinary auto-put path remains
authoritative; the verifier may accept the skipped candidate only when the
complete observed combat subset matches it and the ordinary put-back does not.
Witnesses: FIDL01308, FIDL01620. FIDL01461 last-card Warcry with Unceasing Top
is ordinary auto-place plus Top redraw, not this skip.
