# Neow transform obtain lifecycle

## Evidence

The fresh terminal FIDL01579 trace selects Neow `Transform a Card`, selects a
Defend, and confirms the grid. The confirm response is still the Neow Event
Leave screen with the selected source removed and no replacement in the deck;
the following Leave response contains the deterministic `Perfected Strike`.
The existing Neow transform helper and RNG trace produce that same replacement
from the seed and selected source, so this boundary is timing, not an RNG or
visibility mismatch. Neow's source path uses `ShowCardAndObtainEffect` for the
replacement.

## Decision

Both one- and two-card Neow transforms remove selected source instances at
grid confirmation and queue typed pending obtains. `PendingNeowTransform`
retains the exact source instances, Neow RNG endpoint, and construction-time
Omamori counter. Validation recomputes the replacement from the run seed,
source cards, card-add relic rules, and pending-obtain provenance. The stage-2
Neow Leave action flushes the pending obtain. No observed card identity or
trace-specific timing is authoritative.
