# Bronze Orb Stasis candidate order (do not ship unsorted / BASIC-exclude)

## Observation

Several floor-33 END first-divs (FIDL01391, 01507, 01636, 01784, 01826, 01827)
are Bronze Orb Stasis: a card leaves every player pile while an orb gains
Stasis. FIDL01391 steals Body Slam from a 5-card draw
`[Strike, Defend, Cleave, Body Slam, Flex]`.

## Rejected change

Replacing the name/cardID sort in `take_random_card_of_rarity` with raw pile
order, and treating Strike/Defend/Bash as Java `BASIC` (not Common), made
FIDL01784 and FIDL01827 complete-pass but dropped the corpus from 432 to 350
complete-passes. The shipped sort + reward-rarity Common walk is what the
green Automaton traces require.

Do not reintroduce that pair of changes without a source-backed rule that
keeps those 82 traces green.
