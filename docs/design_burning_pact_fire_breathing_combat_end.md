# Burning Pact Fire Breathing combat end

`BurningPact` CONFIRM exhausts the selection then draws. Each status/curse
draw pulses `FireBreathingPower` immediately (`DamageAllEnemiesAction`). That
can kill the last monster before any remaining bot-queued on-exhaust
follow-up exists, so `process_internal_queue` never runs.

`apply_exhaust_select_confirm` already opens rewards when combat is `Won`
(Juggernaut lethal on Feel No Pain). Exhaust CONFIRM must therefore settle
Won/Lost from current HP after the pact draws, using the same already-won
Burning Blood guard as nested PlayTop.

FIDL01773: Spheric Guardian at 2 HP, Burning Pact exhausts Strike, draws a
status, Fire Breathing 6 ends the fight on CONFIRM (`COMBAT_REWARD`).
