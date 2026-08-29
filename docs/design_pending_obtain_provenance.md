# Deferred visual obtain provenance

`ShowCardAndObtainEffect` is authoritative at construction time for Omamori:
the target consumes one charge before the effect is published, and a blocked
curse never enters `masterDeck`. The simulator therefore records each deferred
visual obtain's exact source card, pre-effect Omamori ownership, and pre-effect
used-charge counter. Validation recomputes the blocked/unblocked result and
requires the exact source sequence; it never derives a source card from an
observation.

The strict FIDL01372 witness is the Hypnotizing Colored Mushrooms `Eat` path:
step 191 exposes Mushrooms, step 192 shows the returned `Leave` page with no
Parasite in the deck and Omamori changing from two remaining charges to one.
The target `Mushrooms.buttonEffect` constructs one Parasite and does not draw
from any RNG stream on this path. The pending provenance is therefore valid,
while an ownerless, source-mismatched, or counter-mismatched pending state
fails closed.
