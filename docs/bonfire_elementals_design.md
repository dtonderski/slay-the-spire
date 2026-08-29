# Bonfire Elementals design note

The event is modeled as three event stages: an intro `Continue` choice, an
`Offer a card` choice that opens a purge grid, and a final `Leave` choice after
the selected card resolves. The selected card is removed from the deck before
the rarity-dependent reward is applied.

The game can award `Spirit Poop` for offering a curse. The simulator does not
model an effect-bearing relic for it, so it is represented as a serialized
`RelicKey` with no combat or run effect. If it is already owned, the game gives
`Circlet` instead.

Card outcomes follow the decompiled event: basic cards have no additional
effect; common and special cards heal 5; uncommon cards fully heal; and rare
cards increase max HP by 10 and fully heal. Bottled cards are excluded from the
grid.
