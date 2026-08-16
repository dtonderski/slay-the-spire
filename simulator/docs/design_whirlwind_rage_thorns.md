# Whirlwind Rage block before deferred hits

`AbstractPlayer.useCard` calls `card.use()` then constructs `UseCardAction`,
whose constructor runs `RagePower.onUseCard` (`addToBot` `GainBlockAction`).

`Whirlwind.use()` only queues `WhirlwindAction`. That wrapper later
`addToBot`s each `DamageAllEnemiesAction`. After `use()` + Rage the queue is
`[WhirlwindAction, GainBlock, UseCardAction]`; the wrapper then appends the
hits behind Rage. Spiker `ThornsPower.onAttacked` `addToTop`s onto a player
who already has the Rage block.

A normal Strike still queues `DamageAction` in `use()`, so Rage stays after
those hits. Do not move Rage in front of immediate attack damage.

FIDL01782 PLAY 1262: Rage 3 + Whirlwind X=2 into 15-thorn Spiker is 27 HP
(3 block consumed) not 30 HP with leftover block.
