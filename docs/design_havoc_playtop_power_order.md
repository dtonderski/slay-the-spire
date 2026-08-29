# Havoc PlayTop power application vs outer settlement

## Scope

When Havoc/Mayhem/Distilled Chaos force-plays a top card, nested `card.use()`
`addToBot` power applications (Feel No Pain, Inflame, …) must not resolve before
the outer source `UseCardAction` settles.

## Evidence

- Target action manager: `Havoc.use` `addToBot`s `PlayTopCardAction`, then
  `useCard` `addToBot`s the outer `UseCardAction`. `PlayTopCardAction`
  `addToTop`s the forced card play while that outer settlement is already queued.
- Nested `FeelNoPain.use` `addToBot`s `ApplyPowerAction`, which therefore lands
  *after* the outer Havoc exhaust under Corruption.
- FIDL00253 step 1135: Havoc PlayTop `Feel No Pain+` leaves `Feel No Pain` amount
  4 active with **block 0** (FNP was not active for the Havoc exhaust). Dead Branch
  still generates into hand from that exhaust.
- Resolving nested `GainFeelNoPain` inside the PlayTop expansion made FNP active
  before Havoc exhausted and produced block 4.

## Force-played True Grit+

FIDL00253 steps 1327–1330: Havoc PlayTop True Grit+ opens exhaust select with
True Grit+ **not** yet in exhaust. On CONFIRM, True Grit+ exhausts (FNP/Dead
Branch), while the selected card is absent from every serialized pile until the
next END flushes it into discard (`pending_hidden_hand_card_until_end_turn`).

## Implementation

- `PlayTopDrawCard` splits nested power-gain actions out of the immediate nested
  queue and returns them as parent follow-ups after outer `MoveCard` settlement.
- Force-play True Grit+ defers source `MoveCard` like Exhume; `confirm_true_grit_select`
  exhausts the parked source and parks the selection as a hidden end-turn card.

## Force-played True Grit+ selection (FIDL00253 vs FIDL00238)

FIDL00253 CONFIRM: True Grit+ exhausts (FNP/Dead Branch once); the selected card is
absent from every pile until the next END flushes it to discard.

FIDL00238 CONFIRM: both True Grit+ and the selected Power Through+ appear in
exhaust with two FNP procs. That second exhaust-on-confirm path is still open;
the FIDL00253 limbo selection model is retained so the promoted witness stays green.


## Sadistic Nature vs Malleable (FIDL00242)

`SadisticNaturePower.onApplyPower` queues `DamageAction` with `addToBot`, so it
resolves after same-card `MalleablePower`/`CurlUpPower` `onAttacked` bot block.
Sadistic damage is returned as `DealUnmodifiedDamage` follow-ups from debuff
applies rather than resolving inline.
