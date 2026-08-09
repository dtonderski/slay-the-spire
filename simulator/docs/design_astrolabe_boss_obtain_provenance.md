# Astrolabe boss obtain provenance

A boss-reward Astrolabe removes its selected source instances when the grid
resolves, but its three `ShowCardAndObtainEffect` results are still pending
until the boss chest `PROCEED` boundary. The simulator records those exact
source instances and the pre-transform misc-RNG and Omamori counters in
`PendingAstrolabeTransform`; it does not infer the cards from an observation.

Pending validation recomputes each transform and upgrade from that provenance,
checks that every source is a valid instance absent from the current deck,
checks the resulting RNG and Omamori counters, and requires the exact pending
content sequence (including Omamori-blocked results). The provenance is
accepted only while an Astrolabe owns the Neow Leave or boss-chest boundary.
Generic event-transform provenance remains independent.

The boss boundary is exercised by the FIDL01249 strict replay through its
Astrolabe grid and chest state; the replay stops later at the pre-existing
Searing Blow metadata boundary.
