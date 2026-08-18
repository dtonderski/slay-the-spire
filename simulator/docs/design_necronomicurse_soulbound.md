# Necronomicurse soulbound exhaust

## Source behavior

`Necronomicurse.triggerOnExhaust` queues `MakeTempCardInHandAction(new
Necronomicurse())`. The exhausted instance stays in the exhaust pile; a new
combat-only copy is created in hand (or discard if the hand is full). Blue
Candle can play the unplayable curse first: lose 1 HP, then move it to exhaust,
which fires this trigger.

`CardGroup.moveToExhaustPile` calls relic `onExhaust`, then power `onExhaust`,
then `card.triggerOnExhaust`. Dark Embrace therefore queues `DrawCardAction`
before Necronomicurse queues `MakeTempCardInHandAction`. Exhaust bot follow-ups
run that draw first, then `AddGeneratedCardToPile` for the replacement
(FIDL01511 PLAY 1336: Iron Wave then the new curse).

Sever Soul's `ExhaustAllNonAttackAction` still snapshots before those bot
actions, so the replacement is not in the first exhaust batch. Necronomicon's
second `use()` snapshots it (FIDL01518 Feel No Pain).

## Evidence

- FIDL01511 step 478: Blue Candle plays Necronomicurse from a five-card hand.
  After CONFIRM-equivalent PLAY, HP drops by 1, the original curse is in
  exhaust, and a new Necronomicurse is at the back of the remaining hand.
- FIDL01511 step 1336: Dark Embrace draws Iron Wave, then the replacement
  curse is appended in the real game.

## Non-goals

- Do not change Blue Candle HP-loss or Rupture observation.
- Do not rewrite Necronomicon obtain / master-deck ownership.
- Do not treat the replacement as a master-deck card.
