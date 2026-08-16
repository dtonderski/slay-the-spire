# Relic onUnequip reverses pickup energy

## Rule

Energy relics add `energy_per_turn` in `gain_relic`. Java `AbstractRelic.onUnequip`
subtracts that bonus when the relic is lost (N'loth, Moai Head idol trade).

## Bug

`remove_relic_key` only dropped the relic from the list. FIDL01680 offered
Velvet Choker to N'loth, then entered combat at 3 energy; sim kept 4.

## Fix

`RunState::lose_relic_key` subtracts the same pickup bonus used on gain.
