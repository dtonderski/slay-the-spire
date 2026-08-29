# Fallible Run Card Acquisition

## Problem

Run card acquisition and preview passed content through egg relics before adding
it to the deck. Missing definitions were returned unchanged, allowing public
`gain_deck_card`/`add_deck_card` calls and imported reward choices to preserve
unknown content as plausible run state. Transform and reward paths duplicated
this infallible projection.

## Decision

Persistent acquisition validates a known card definition unconditionally.
Card-add relic projection requires type/upgrade metadata only when at least one
egg relic could transform the card. Truly unknown content then returns
`UnknownContent`; a known Prismatic Shard reward-only identity without modeled
type metadata returns `UnsupportedMechanic`. Without eggs, such identities may
remain display-only reward choices. Matching eggs still upgrade only cards with
an available upgrade link; already-upgraded and inherently unupgradable content
is left unchanged, preserving target egg semantics.

The public persistent-card mutation also validates the incoming instance and
rejects duplicate deck IDs before Omamori, egg projection, or card-obtain relic
effects. Combat-local metadata cannot enter the run deck through this API.

Deck addition remains transactional through its existing cloned-state boundary.
Pandora's Box, Neow/event transforms, generated reward choices, and reward
previews propagate projection errors before publishing their new grid, deck, or
reward state.

## Verification

Regressions require unknown content, duplicate IDs, and combat-local metadata to
fail without changing the run or applying card-obtain relic side effects.
Existing egg, transform, reward, snapshot, and permanent replay coverage remains
the behavioral compatibility gate.
