# Nemesis Intangible before Cube Fire Breathing

## Source behavior

`Nemesis.takeTurn` queues its `DamageAction`s, then `ApplyPowerAction(IntangiblePower)`
when `!hasPower("Intangible")`, then `RollMoveAction`.

`RunicCube.wasHPLost` `addToTop`s `DrawCardAction`. `FireBreathingPower.onCardDraw`
`addToBot`s `DamageAllEnemiesAction`, so that damage resolves after the queued
Intangible apply (FIDL01313 END 1058: Nemesis 42→39, not 42→34).

## Non-goals

- Do not move Runic Cube draws before the remaining multi-hit `DamageAction`s.
- Do not change Intangible decay on a Nemesis that already has the power.
