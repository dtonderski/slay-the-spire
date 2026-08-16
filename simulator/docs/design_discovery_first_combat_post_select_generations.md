# FIDL01561 Sludge Void vs Discovery SuperFastMode pulses (parked)

## Witness

FIDL01561 END 2132 first-divs after a matching Clothesline. Real leftover
has energy 4, a six-card hand without Void, Void at leftover draw index 9,
and Awakened One at 200 HP. Sim draws Void (energy 3, seven-card hand,
Fire Breathing 10).

`cardRandomRng` at that insert is three draws (one `generateCardChoices`)
ahead of the leftover index 9. Those three draws are a second discarded
post-select generation on the fight's only Discovery (`PLAY` 2057 /
`CHOOSE` 2058).

## Why this is not a core rule

`DiscoveryAction.update()` generates at the start of every update. The
installed SuperFastMode config usually yields one post-select pulse
(`1` generation, or `2` when another Discovery is still in hand —
FIDL01630).

The same first-retrieve-against-Awakened-One, empty-follow-up shape
needs **two** pulses on FIDL01561 and **one** pulse on FIDL01665
(Wild Strike+ Wound insert at 1410). No pre-`CHOOSE` simulator field
distinguishes those two plays. A global or Awakened-One-gated
two-generation retrieve gains 01561 and loses 01665.

A verifier candidate at `CHOOSE` cannot choose: both pulse counts
publish the same compared combat subset (the selected card is added;
only the hidden `cardRandomRng` stream differs). Using the later Void
or Wound location to pick the pulse count would hydrate RNG from
observed piles.

## Decision

Keep the existing `1` / `2`-if-another-Discovery-in-hand retrieve
model. Do not special-case 01561. The leftover Void insert stays
queued behind Runic Cube's `addToTop` draw (`random(size-1)`).
