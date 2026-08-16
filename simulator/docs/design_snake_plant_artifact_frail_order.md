# Snake Plant Spores vs Artifact

`SnakePlant.takeTurn` queues `ApplyPowerAction(FrailPower)` then
`ApplyPowerAction(WeakPower)`. One Artifact stack (Clockwork Souvenir)
consumes Frail; Weak remains.

Maw Roar / Collector Mega Debuff still apply Weak then Frail
(FIDL01475 / FIDL01632). Do not invert the shared
`ApplyPlayerFrailAndWeak` order globally.

FIDL01810: after Spores, Flame Barrier+ is 16 block, not 12 (Frail 25%).
