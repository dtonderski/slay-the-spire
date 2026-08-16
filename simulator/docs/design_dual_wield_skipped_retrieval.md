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

A second SuperFastMode skip completes before the select screen returns
non-Attack/Power cards that were dropped from the published hand (FIDL01816
Rage/Wound, FIDL01715 Sentinel, FIDL01722 Impervious, FIDL01255 Purity).
Those cards stay off every combat pile. The verifier tries the restore
candidate first, then this no-restore candidate, and keeps whichever
stable post-CONFIRM subset matches. Successful retrieval still restores
those cards and remains authoritative when observation includes copies
(FIDL01438).

Ordinary hand Dual Wield (source still delayed in hand) keeps the core copy
path authoritative; successful multi-select retrieval under force-exhaust also
keeps the core path when observation matches it.

## Singleton force-play

`DualWieldAction` does not open a select when only one Attack/Power remains.
Havoc PlayTop still stages Dual Wield into hand, so `source_started_in_hand`
is not a force-play signal; auto-confirm uses `play_top_force_exhaust`.
`MakeTempCardInHandAction` appends copies without moving the original card
(FIDL01368: hand stays `Feel No Pain, Seeing Red, Feel No Pain`).

## Force-play multi-select deferral (FIDL00242)

When Havoc/Mayhem force-plays Dual Wield with **more than one** Attack/Power
eligible, source exhaust is deferred until CONFIRM (not early MoveCard). That
lets Dark Embrace draw on CONFIRM after Dual Wield exhausts (Bite in FIDL00242).

Singleton force-play still early-exhausts and auto-confirms.

On multi-select open, Dual Wield is parked in limbo and non-eligible hand cards
non-Attack/Power cards leave the serialized combat hand while the selection
is open. CommunicationMod only exposes Attack/Power candidates. Skills and
already-combat-generated status/curses return on CONFIRM; deck-owned curses
remain owned by the selection screen and return as well (Parasite in FIDL01438).
Combat-generated Shame still disappears for the rest of combat in FIDL00242;
its `combat_only` instance is not a deck-owned card to restore.

Monster `MakeTempCardInDiscardAction` / `MakeTempCardInDrawPileAction` status
cards (Deca Dazed, Hexaghost Burns) are `combat_only`. Dual Wield restores
those shuffled statuses with skills (FIDL01246 Dazed before Shockwave/Shrug).
A `combat_only=false` Dazed is dropped as if it were a deck-owned status.

`confirm_dual_wield_select_skipped_retrieval` settles limbo Dual Wield (exhaust +
on-exhaust) without creating copies and parks the selection until end-turn
discard flush.

## Force-exhaust vs hand-play discard (session continuation)

Dual Wield's card definition does **not** use the exhaust keyword. Hand-play
settles to discard (trace-session-8). Havoc/Mayhem/Distilled Chaos PlayTop sets
`CombatState.play_top_force_exhaust_active` so CONFIRM force-exhausts the source
(FIDL00242, random-fidelity-9074). The flag must survive `play_top_draw_card_queue`'s
state clone and must not be cleared before the nested AwaitHandSelect runs.

## Multi-select non-eligible cards

Force multi-select drops non-Attack/Power from the serialized hand while the
selection is open. Skills and deck-owned curses are parked on
`HandSelectState.dual_wield_restore_on_confirm` and restored on CONFIRM (same
UUIDs; 9074cf38 Defend_R; Parasite FIDL01438). Combat-generated status/curses
such as Shame remain dropped for the rest of combat (FIDL00242). Parked restore
cards participate in instance-ID reservation so Dual Wield copies cannot collide
with them.
