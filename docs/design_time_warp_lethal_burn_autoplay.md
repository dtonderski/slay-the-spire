# Time Warp 12th-card kill still autoplays Burns

`TimeWarpPower.onAfterUseCard` at 12 calls `callEndTurnEarlySequence` even
when that card killed the last enemy. End-turn Burns (`dontTriggerOnUseCard`
THORNS) resolve before DeathScreen.

FIDL01371: Anger+ is the 12th card and lethal. Real COMPLETE HP is 8500
(8502 minus Burn 2). Skipping the forced end because no monster is alive
leaves 8502.

Do not start a monster turn or next-hand draw after that lethal.
