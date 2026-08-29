# Pain card-play HP loss before selection screens

## Evidence

`Pain.triggerOnOtherCardPlayed` constructs `LoseHPAction(player, player, 1)`
and calls `addToTop`. Therefore the loss is a card-play follow-up, not part of
Pain's own `use`, and it must be inserted ahead of a card's queued selection
boundary. FIDL01308 (Warcry+) and FIDL01288 (Burning Pact) both showed the
same one-HP lag when the simulator left that follow-up pending under the open
selection screen; Centennial Puzzle's draw was consequently late as well.

## Decision

When a card-sourced `LoseHp` follow-up is queued, insert it before the first
player-selection internal action. If no selection is queued, preserve the
existing queue placement, including the established Rupture ordering. This is
a shared action-queue rule and does not inspect seeds, trace IDs, observations,
or card-specific repair tables.

## Verification

The Warcry/Pain regression asserts HP loss before hand selection. The
Burning Pact/Pain/Centennial Puzzle regression asserts HP loss and the relic
draw before exhaust selection. Strict replay advances FIDL01308 through step
472 (later Pommel Strike+ monster-strength boundary) and FIDL01288 through step
433 (later discard-order boundary).
