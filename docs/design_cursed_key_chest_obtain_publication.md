# Cursed Key chest curse publication

Cursed Key's non-boss chest path creates a random normal curse through
`ShowCardAndObtainEffect`. CommunicationMod can publish the initial
`OpenChest -> COMBAT_REWARD` frame before that deck mutation is visible, then
publish the curse on the next reward poll. FIDL01385, FIDL01499, FIDL01579,
and FIDL01679 are witnesses; the target deck gains Shame, Decay, Normality,
and Pain respectively on the following reward action.

The core keeps the deterministic obtain eager and authoritative. Strict replay
accepts the old-deck reward projection only when the action is `OpenChest`, the
source owns Cursed Key, exactly one new deck card is the generated curse, and
all other reward fields match. The next source reward action is compared
against the canonical simulator state with the curse present. No observed deck
contents are copied into simulator state.
