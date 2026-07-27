# Burning Pact ExhaustAction skipped retrieval

## Source behavior

Vanilla `ExhaustAction` opens `HandCardSelectScreen` and calls `tickDuration()`
on that first update. While the screen is up, `AbstractRoom` skips
`GameActionManager.update`, freezing the action. When CONFIRM closes the
screen, the manager resumes ExhaustAction: if it is already `isDone` (duration
expired on the open frame under a large frame delta — common late in long
Sentry fights with many `ExhaustCardEffect` particles), the retrieval loop that
calls `hand.moveToExhaustPile` on `selectedCards` never runs.

The selected card remains owned by the closed selection screen and is absent
from hand, draw, discard, exhaust, and limbo. `DarkEmbracePower.onExhaust` does
not fire. Queued `DrawCardAction` from Burning Pact and `UseCardAction` for the
played source still resolve.

This is the same action-manager/screen interaction documented for
`PutOnDeckAction` in `design_put_on_deck_skipped_retrieval.md`.

## Verifier contract

Core keeps the normal exhaust + Dark Embrace + draw path authoritative.

When CONFIRM on a Burning Pact exhaust-select settles a stable combat frame that
does **not** match that path, the verifier builds a source-backed candidate from
the pre-CONFIRM state:

1. Remove the selected hand card into `pending_hidden_hand_card_until_end_turn`
   (or leave it untracked under Runic Pyramid, matching retained-hand traces).
2. Draw only the Burning Pact amount (2 or 3).
3. Move the held source card to discard.

No observed card identities or pile order are copied into simulation. The
candidate is accepted only when the stable observed subset matches it
(including selected card absent from observed exhaust). Later commands continue
from the candidate so end-turn settlement of the pending card participates in
discard→draw shuffle correctly.

Core `end_player_turn` settles `pending_hidden_hand_card_until_end_turn` into
discard only when the visible hand was non-empty at END (same leftover
`selectedCards` window as put-on-deck skipped retrieval). Empty-hand ENDs hold
the card outside every pile through the next refill so it does not contaminate
the discard→draw shuffle (`random-fidelity-c60c2349aa8da68d` step 237).

## Evidence

- `random-fidelity-46eca3ff50276214` / `131acce58bb62226` step 252 CONFIRM:
  selected Shrug UUID missing from all piles; hand gains only BP draws; after
  non-empty-hand END the UUID reappears via shuffle/draw.
- Same combat step 227 CONFIRM (earlier, lighter exhaust load): normal exhaust
  + Dark Embrace draw matches core.
- `random-fidelity-c60c2349aa8da68d` step 234 deferred CONFIRM → spend hand →
  empty-hand END 237: stuck Thunderclap stays out of hand/draw/discard.
