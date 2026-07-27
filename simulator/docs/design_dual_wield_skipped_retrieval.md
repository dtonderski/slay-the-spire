# Dual Wield skipped retrieval frame

## Source behavior

Vanilla `DualWieldAction` opens `HandCardSelectScreen` when more than one
Attack/Power is eligible. On open it calls `tickDuration()`. If that duration
completes before CONFIRM (large frame delta under CommunicationMod load or
heavy VFX), `GameActionManager` advances past `DualWieldAction` without the
retrieval update that queues `MakeTempCardInHandAction` copies.

The selected card remains owned by the closed selection-screen object and is
absent from hand/draw/discard/exhaust/limbo in CommunicationMod combat state.
`UseCardAction` still settles Dual Wield (force-exhaust under Havoc → exhaust).
Later, end-turn `DiscardAction` processes leftover `selectedCards` when
`wereCardsRetrieved` is still false and moves the stuck card into discard.

This matches the put-on-deck / Burning Pact skipped-retrieval family
(`design_put_on_deck_skipped_retrieval.md`,
`design_burning_pact_exhaust_skipped_retrieval.md`).

## Verifier contract

The normal core Dual Wield confirm still creates temporary copies. The
skipped-retrieval result is a separate source-backed transition candidate,
eligible only when:

1. Hand select purpose is `DualWieldCopy` with a selected index
2. Dual Wield's source is already in exhaust or discard (force-exhaust top-draw
   path: Havoc / Mayhem / Distilled Chaos)
3. The stable post-CONFIRM combat subset matches the candidate

The candidate is rebuilt only from the pre-CONFIRM simulator state: close the
hand select without copies, park the selected card in
`pending_hidden_hand_card_until_end_turn`. No observed pile/RNG hydration.
End-turn cleanup already appends that pending card to discard.

Ordinary hand Dual Wield (source still delayed in hand) keeps the core copy
path authoritative; successful multi-select retrieval under force-exhaust also
keeps the core path when observation matches it.
