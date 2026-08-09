# Gambling Chip skipped retrieval

## Source behavior

Gambling Chip (and Gambler's Brew) open a multi-card hand selection and then
discard the chosen cards and draw that many replacements. The selection uses the
same hand-select screen / action-manager interaction as `ExhaustAction` and
`PutOnDeckAction`: under a large frame delta the owning action can already be
`isDone` when CONFIRM closes the screen, so the post-confirm retrieval loop that
moves `selectedCards` into discard and queues the replacement draws never runs.

Selected cards therefore leave the published hand (the closed selection screen
still owns them) but are absent from hand, draw, discard, exhaust, and limbo.
Replacement draws do not run. A later non-empty end-turn discard settles the
stuck `selectedCards` into discard in selection order; empty-hand ENDs keep them
outside every pile so they do not contaminate the discard→draw shuffle.

## Verifier contract

Core keeps the ordinary discard-then-draw path authoritative.

When CONFIRM on a Gambling Chip exhaust-select settles a stable combat frame
that does **not** match that path, the verifier rebuilds a source-backed
candidate from the pre-CONFIRM state:

1. Remove the selected hand cards (selection order) into
   `pending_hidden_hand_card_until_end_turn`.
2. Do not discard them and do not draw replacements.
3. Accept the candidate only when the complete observed combat projection
   matches exactly.

No observed card identities or pile order are copied into simulation. Later
commands continue from the candidate so end-turn settlement of the pending cards
participates in discard→draw shuffle correctly.

## Selection-screen replacement

`HandCardSelectScreen.open` calls its source `prep()` routine first. The
routine clears the shared screen's `selectedCards` and resets
`wereCardsRetrieved`. Therefore, if another interrupted hand-selection action
opens before a prior skipped candidate settles, the prior screen-owned cards
are lost and the new candidate replaces the pending hidden selection. The
verifier applies that rule generically to the source-backed skipped-selection
candidates; it does not merge or preserve stale cards from the replaced screen.

## Evidence

- FIDL01248 step 139 CONFIRM: selected Strike+ UUID missing from all piles;
  draw pile unchanged; discard empty; after non-empty END 141 the UUID is in
  discard with the rest of the hand.
- FIDL01248 steps 168–170: select entire hand → empty-hand END keeps five
  hidden cards out of discard while drawing the next hand; the following
  non-empty END settles all ten cards into discard.
