# Ironclad Card Definition Audit

Date: 2026-07-01

## Scope

This audit reviews whether the local simulator models Slay the Spire 1 Ironclad
cards at the definition and combat-dispatch level. It follows up on a broad
online comparison pass against the card data embedded by `sts.gg/v1/cards` and
an independent sub-agent review.

Primary local files checked:

- `simulator/crates/sts_core/src/content/cards.rs`
- `simulator/crates/sts_core/src/content/reward_pool.rs`
- `simulator/crates/sts_core/src/combat/card_effects.rs`
- `simulator/docs/m32a_cards_matrix.md`

The public comparison baseline used for printed card values was the 374-card
STS1 data array embedded in `https://sts.gg/v1/cards` via the Nuxt asset
`https://sts.gg/_nuxt/BF9KVfPQ.js`. Fandom pages were attempted first but were
blocked by anti-bot challenge pages from command-line tooling.

## Summary

- All Ironclad base cards have local `CardDefinition` coverage.
- All first-upgrade Ironclad variants now have local `CardDefinition` coverage.
- All Ironclad base and first-upgrade cards are playable through
  `play_card_queue`, either by explicit match arms or generic attack/block
  fallbacks.
- The high-confidence definition mismatches found in this audit have been fixed.
- `Searing Blow` now has instance-level repeated-upgrade support: the first
  upgrade changes the content id to `Searing Blow+`, later upgrades keep that
  content id and increment `CardInstance.searing_blow_upgrades`.

## Base Ironclad Coverage

The local reward pool includes the target Ironclad combat-reward base-card
inventory in `IRONCLAD_REWARD_ENTRIES`, and the local `ALL_CARDS` inventory
contains the Ironclad starter cards plus reward/transform base definitions.

Combat dispatch in `play_card_queue` covers the base Ironclad set. Many cards
are routed through explicit handlers, while simple damage or block-only cards
can also fall through to generic attack or skill queues.

Conclusion: base Ironclad definition and playable-combat coverage is present.
This does not imply exact action-manager, RNG, UI, or trace parity.

## Upgraded Ironclad Definitions

The following first-upgrade Ironclad variants were missing and have been added
to `ALL_CARDS`, `upgrade_content_id`, and card type/rarity lookup:

| Added upgraded card | Notes |
| --- | --- |
| `Iron Wave+` | upgraded block and damage |
| `Body Slam+` | upgraded cost |
| `Clash+` | upgraded damage |
| `Thunderclap+` | upgraded damage |
| `Clothesline+` | upgraded Weak amount |
| `Wild Strike+` | upgraded damage |
| `Heavy Blade+` | upgraded strength scaling |
| `Perfected Strike+` | upgraded strike-name bonus |
| `True Grit+` | upgraded block and chosen exhaust |
| `Power Through+` | upgraded block |
| `Reckless Charge+` | upgraded damage |
| `Hemokinesis+` | upgraded damage |
| `Intimidate+` | upgraded Weak amount |
| `Pummel+` | upgraded hit count |
| `Disarm+` | upgraded Strength reduction |
| `Rage+` | upgraded block-per-attack amount |
| `Entrench+` | upgraded cost |
| `Sentinel+` | upgraded block and exhaust energy |
| `Bloodletting+` | upgraded energy gain |
| `Carnage+` | upgraded damage |
| `Dropkick+` | upgraded damage |
| `Fire Breathing+` | upgraded damage trigger amount |
| `Ghostly Armor+` | upgraded block |
| `Sever Soul+` | upgraded damage |
| `Feel No Pain+` | upgraded block-per-exhaust amount |
| `Dark Embrace+` | upgraded cost |
| `Impervious+` | upgraded block |

`Searing Blow` is represented by base `Searing Blow` and first-upgrade
`Searing Blow+`. The first upgrade uses the correct 16 damage baseline, and
later upgrades are represented by the card instance upgrade count with the
target damage sequence 12, 16, 21, 27, ...

## Existing Ironclad Definition Differences

These local definition differences existed before the implementation pass and
have been corrected:

| Card | Local definition | Public baseline |
| --- | --- | --- |
| `Twin Strike+` | 6 damage per hit | 7 damage per hit |
| `Dark Embrace` | cost 1 | cost 2 |
| `Pommel Strike+` | 12 damage | 10 damage and 2-card draw |
| `Dual Wield` | marked Exhaust | does not Exhaust |
| `Dual Wield+` | cost 0 and marked Exhaust | cost 1 and does not Exhaust |
| `Searing Blow+` | 20 damage | first upgrade should be 16 damage; later upgrades scale from instance count |
| `Infernal Blade` / `Infernal Blade+` | not marked Exhaust in definition | Exhausts |
| `Sword Boomerang+` | 4 damage, apparently per hit | 3 damage, 4 hits |

`Whirlwind`, `Whirlwind+`, `Transmutation`, and `Transmutation+` still store
printed X-cost as `0` in `CardDefinition.cost`; this is an intentional local
representation because `CardDefinition.cost` is a `u8`. Playability uses
explicit X-cost helpers instead of the scalar cost field.

## Definition And Pool Consistency Notes

`Dark Embrace` now has cost 2 and Rare rarity consistently across
`card_type_and_rarity` and the Ironclad reward pool.

The earlier audit pass also found non-Ironclad/status representational issues
such as curses being modeled as `CardType::Status`, but those are outside this
Ironclad-card-focused report except where they affect Ironclad runs through
curse rewards or events.

## Corrections To Prior Framing

The prior shorthand, "all Ironclad base cards are modeled; upgraded-card parity
has rough edges," is now stale. The current statement is:

All Ironclad base cards and first-upgrade variants have local definitions and
combat dispatch. Repeated-upgrade `Searing Blow` behavior is modeled through
per-card instance upgrade metadata because it cannot be represented by a single
content-id-to-content-id edge.

One stale caveat from older docs should also be treated carefully: `Seeing Red`
is currently marked Exhaust in the local definition and routed through
`seeing_red_queue`, so older notes implying local discard behavior appear out of
date.

## Remaining Follow-Up

1. If printed card metadata becomes a user-facing API, consider adding an
   explicit cost enum so X-cost cards can be surfaced as `X` while retaining the
   existing playability logic.

## Verification Performed

- Inspected local card definitions and `ALL_CARDS` inventory.
- Inspected Ironclad reward-pool base-card entries.
- Inspected `play_card_queue` dispatch coverage.
- Compared local definitions against the `sts.gg` embedded STS1 card dataset.
- Requested and incorporated an independent sub-agent review.
- Ran `cargo test -p sts_core combat::transition`.
