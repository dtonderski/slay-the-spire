# Dagger explode suicide waits behind a lethal hit

`SnakeDagger.takeTurn` EXPLODE queues `DamageAction` then
`LoseHPAction(self, currentHealth)`. A lethal hit constructs DeathScreen and
freezes that later suicide.

FIDL01796: the last living Dagger's 25 kills the player at 15 HP. Real
GAME_OVER still shows the Dagger at 22. Applying suicide before player
damage leaves `monsters[0].current_hp 22 != 0`.

Do not start Reptomancer's following takeTurn after that death.
