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
FIDL01255 colorless hand). Donu/Deca with 6+ remaining takes two pulses
(FIDL01431: Metamorphosis then generates Blood for Blood / Clash / Immolate).
Remaining hands of 3–5 against Awakened One
are one pulse while another enemy is still alive (FIDL01665 Cultist+AO).
Five remaining cards against **solo living** Awakened One take two pulses
(FIDL01357 leftover Sludge Void at draw index 7, not 4). Two remaining cards
that include a status take two pulses (FIDL01357: Defend+Dazed then Wild Strike
Wound at draw index 21, not 0). Donu/Deca does not take that extra pulse
(FIDL01622: Clash+Dazed then Wild Strike Wound at draw index 21, not 11).
Two remaining non-status cards stay one pulse
(FIDL01614 Infernal Blade). Three remaining cards against a **single**
living enemy that is not Awakened One / Donu / Deca / Snecko burn **no** leftover
pulse versus Spire Growth (FIDL01680 Reckless Charge Dazed at draw index 5, not 2). Transient, Jaw Worm, and Writhing Mass keep one pulse. Snecko still
burns one pulse (FIDL01622). Another Discovery still in hand still needs
two pulses (FIDL01630 first pick). Playing a Magnetism-generated Discovery
among the first two cards of the turn needs two pulses when the remaining
hand is smaller than 5 or another Magnetism-generated card is still in
hand (FIDL01787 first-combat Transmutation, remaining 4; later Discoveries
that leave Master of Strategy / other generated cards). Remaining five with
another Magnetism-generated card in hand stays **one** discarded generation
(FIDL01416: next Magnetism is Jack of All Trades; two pulses yield Panic
Button and zero yield Metamorphosis). Remaining hand of
5 with no other generated card is one pulse (FIDL01582: next Magnetism is
The Bomb, not Blind). A lone early-turn retrieve from a 6+ card hand is
one pulse (FIDL01787 Writhing Mass: next Magnetism is Flash of Steel, not
Good Instincts). The same Magnetism-generated source later in the turn
stays one pulse (FIDL01255 colorless hand; FIDL01623 Jack of All Trades
turn). Skipped retrieval still burns nothing. Four remaining versus Time
Eater stay one leftover pulse. The former claim that later Reckless Charge
inserts consume four same-bound `addToRandomSpot` rolls was removed: target
bytecode performs exactly one insertion and one card RNG draw in the effect
constructor.

A global or remaining-hand-only two-generation retrieve regresses those
already-green traces. A CHOOSE-time candidate cannot distinguish 1 vs 2
pulses: both publish the same compared combat subset.

Havoc PlayTop force-exhausts the Discovery source (`ExhaustSpecificCardAction`
on top of `DiscoveryAction`) before the reward screen. After CHOOSE that path
burns **six** discarded generations (FIDL01614 Infernal Blade: Blood for Blood
rather than Perfected Strike). Hand-played Discovery on the same run stays
one pulse (steps 325 and 366, both followed by a matching Infernal Blade).
Mayhem PlayTop does not set `source_card_force_exhaust`. Leftover SuperFastMode
already drained the extra Magnetism pulse before CHOOSE, so early-turn
Magnetism retrieve stays one pulse even when other generated cards remain
(FIDL01255 Deep Breath / Good Instincts, not Swift Strike / Hand of Greed).
Hand-played Magnetism Discovery still uses the 1/2 path (FIDL01787).
FIDL01806's Havoc-Discovery has no later compared `card_random` in that combat.

Hex parks a Dazed `addToRandomSpot` until CHOOSE. With two living enemies that
insert needs two discarded generations (FIDL01614 Cultist+Chosen: Dazed at
draw index 3). Solo Chosen stays one generation; two would desync later Hex
inserts (FIDL01561).

## Current decision

The encounter- and hand-shape pulse table above came from the legacy
pre-collection.2 cohort and is not an authoritative target rule. Production
replay now models one resumed post-select `generateCardChoices` call for an
ordinary retrieved Discovery and none for a skipped retrieval. Any different
count in a clean trace must remain a visible RNG divergence until source-backed
action-queue settlement explains it. Do not restore the historical table or
hydrate an insertion index from an observed pile.
