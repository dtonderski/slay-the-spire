# Discovery post-select generations and leftover Void (FIDL01561)

## Witness

FIDL01561 END 2132 first-divs after a matching Clothesline. Real leftover
has energy 4, a six-card hand without Void, Void at leftover draw index 9,
and Awakened One at 200 HP. Sim with one discarded Discovery generation
draws Void (energy 3, seven-card hand, Fire Breathing 10).

`cardRandomRng` at that insert is one `generateCardChoices` (three draws)
ahead of leftover index 9. Those draws are a second discarded post-select
generation on the fight's only Discovery (`PLAY` 2057 / `CHOOSE` 2058).
After Runic Cube takes the current top card, `addToRandomSpot` uses
`random(size-1)` against the remaining 29-card pile; roll 9 leaves Void
undrawn.

## Source

`DiscoveryAction.update()` calls `generateCardChoices` at the start of
every update. SuperFastMode multiplies `getDeltaTime` (collection fork,
`deltaMultiplier=100`). After CHOOSE the action keeps pulsing until
`tickDuration` exhausts `ACTION_DUR_FAST`.

When Awakened One is in the fight and **6+ cards remain** after the
source left, that settlement takes two post-select pulses (FIDL01561).
The same 6+ remaining-hand shape against Darklings / Giant Head / Champ
/ Time Eater stays one pulse (FIDL01309 Wound insert; FIDL01248 energy;
FIDL01255 colorless hand). Smaller remaining hands against Awakened One
are one pulse (FIDL01665). Another Discovery still in hand still needs
two pulses (FIDL01630 first pick). Skipped retrieval still burns nothing.

A global or remaining-hand-only two-generation retrieve regresses those
already-green traces. A CHOOSE-time candidate cannot distinguish 1 vs 2
pulses: both publish the same compared combat subset.

## Decision

Burn two discarded generations when another Discovery is still in hand,
or when Awakened One is present and `hand.len() >= 6` at retrieve.
Otherwise burn one. Do not hydrate the Void insert index from the
observed leftover pile.
