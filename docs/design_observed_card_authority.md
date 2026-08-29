# Observed card authority

## Problem

Verifier projections historically treated observed card JSON as a best-effort
lookup. Missing or malformed `upgrades` values defaulted to zero, unknown combat
cards disappeared through `filter_map`, and a known base card with an
unmodeled upgrade collapsed back to the base identity. Each behavior could turn
missing or divergent observable evidence into a plausible match.

The permanent CommunicationMod corpus contains explicit `id` and `upgrades`
for every authoritative deck, combat-pile, and card-reward entry. There is no
corpus compatibility requirement for the permissive defaults.

## Contract

Trace import requires every deck and projected combat-pile card to provide a
nonblank string `id` and a non-negative `u8` `upgrades` count. Card reward,
shop, and hand-selection schemas retain the same requirement they already
enforced. Malformed card authority is invalid trace input and must fail before
replay or projection.

Projection maps a card to modeled content only when the observed upgrade count
has an exact modeled identity. An upgraded card without such an identity is
kept as a display identity, preferring its nonblank observed name and otherwise
including the upgrade count with its id. Unknown cards follow the same path.
They remain comparison evidence; they are never silently dropped.

Missing collections are still rejected by their owning trace schema. Internal
projection helpers may therefore assert the validated card contract rather
than inventing defaults after import.

## Scope

This change hardens card identity and upgrade authority only. It does not claim
that every display-only card is simulatable, nor does it add new card mechanics.
Unsupported content must remain visible until the simulator models it or emits
an explicit unsupported boundary.
