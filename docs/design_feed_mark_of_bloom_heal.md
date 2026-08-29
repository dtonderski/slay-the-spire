# Feed Max-HP Gain Versus Mark of the Bloom

## Problem

Feed and Feed+ award `increaseMaxHp` on a non-minion kill: +3 unupgraded, +4
upgraded. The current combat path also heals that same amount, including the
Magic Flower combat-heal multiplier. Three Act 3 traces with Mark of the Bloom
(`FIDL01361`, `FIDL01644`, `FIDL01674`) show the max-HP half landing while
current HP stays put. The simulator still heals, so Feed+ is consistently
`observed != simulated` by exactly the upgraded bonus.

Reaper already routes unblocked-HP healing through
`heal_combat_player_with_relics`, which no-ops under Mark of the Bloom. Feed
bypasses that helper and writes `player.hp` directly.

## Decision

Keep Feed's max-HP award on a completed non-minion kill, including a finished
Darkling pack. Apply the current-HP heal only when combat Mark of the Bloom is
unset. Magic Flower still multiplies only the heal half. Red Skull still
resynchronizes after the max-HP change, even when the heal is suppressed.

This matches the existing Singing Bowl contract: Mark of the Bloom blocks
healing, not max-HP gain.

## Verification

A combat unit test kills with Feed+ under Mark of the Bloom and asserts max HP
rises while current HP does not. The three corpus witnesses must advance past
the Feed+ kill. Ordinary Feed / Magic Flower / Darkling pack tests stay
unchanged.
