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

- All Ironclad base cards appear to have local `CardDefinition` coverage.
- All Ironclad base cards appear to be playable through `play_card_queue`, either
  by explicit match arms or generic attack/block fallbacks.
- The larger gap is upgraded-card coverage: many Ironclad upgraded variants are
  not represented as separate local content definitions.
- Several existing upgraded definitions differ from the public baseline.
- Some earlier matrix caveats are stale or too soft; this report treats missing
  upgraded definitions as a concrete inventory gap, not just a parity caveat.

## Base Ironclad Coverage

The local reward pool includes the target Ironclad combat-reward base-card
inventory in `IRONCLAD_REWARD_ENTRIES`, and the local `ALL_CARDS` inventory
contains the Ironclad starter cards plus reward/transform base definitions.

Combat dispatch in `play_card_queue` covers the base Ironclad set. Many cards
are routed through explicit handlers, while simple damage or block-only cards
can also fall through to generic attack or skill queues.

Conclusion: base Ironclad definition and playable-combat coverage is present.
This does not imply exact action-manager, RNG, UI, or trace parity.

## Missing Upgraded Ironclad Definitions

The following upgraded Ironclad variants appear absent as local
`CardDefinition`s, even though their base card exists:

| Missing upgraded card | Base card status |
| --- | --- |
| `Iron Wave+` | base defined and dispatched |
| `Body Slam+` | base defined and dispatched |
| `Clash+` | base defined and dispatched |
| `Thunderclap+` | base defined and dispatched |
| `Clothesline+` | base defined and dispatched |
| `Wild Strike+` | base defined and dispatched |
| `Heavy Blade+` | base defined and dispatched |
| `Perfected Strike+` | base defined and dispatched |
| `True Grit+` | base defined and dispatched |
| `Power Through+` | base defined and dispatched |
| `Reckless Charge+` | base defined and dispatched |
| `Hemokinesis+` | base defined and dispatched |
| `Intimidate+` | base defined and dispatched |
| `Pummel+` | base defined and dispatched |
| `Disarm+` | base defined and dispatched |
| `Rage+` | base defined and dispatched |
| `Entrench+` | base defined and dispatched |
| `Sentinel+` | base defined and dispatched |
| `Bloodletting+` | base defined and dispatched |
| `Carnage+` | base defined and dispatched |
| `Dropkick+` | base defined and dispatched |
| `Fire Breathing+` | base defined and dispatched |
| `Ghostly Armor+` | base defined and dispatched |
| `Sever Soul+` | base defined and dispatched |
| `Feel No Pain+` | base defined and dispatched |
| `Dark Embrace+` | base defined and dispatched |
| `Impervious+` | base defined and dispatched |

`Searing Blow` is also only represented by one local `Searing Blow+` definition.
The real card can be upgraded repeatedly, so a single fixed upgraded definition
does not model the full upgrade chain.

## Existing Ironclad Definition Differences

These are local definitions that exist but differ from the public baseline.

| Card | Local definition | Public baseline |
| --- | --- | --- |
| `Twin Strike+` | 6 damage per hit | 7 damage per hit |
| `Dark Embrace` | cost 1 | cost 2 |
| `Pommel Strike+` | 12 damage | 10 damage and 2-card draw |
| `Whirlwind` / `Whirlwind+` | printed cost stored as 0 | X-cost |
| `Dual Wield` | marked Exhaust | does not Exhaust |
| `Dual Wield+` | cost 0 and marked Exhaust | cost 1 and does not Exhaust |
| `Searing Blow+` | 20 damage | first upgrade should be 16 damage; full card supports repeated upgrades |
| `Infernal Blade` / `Infernal Blade+` | not marked Exhaust in definition | Exhausts |
| `Sword Boomerang+` | 4 damage, apparently per hit | 3 damage, 4 hits |

The `Whirlwind` rows may be an intentional representation shortcut because
`CardDefinition.cost` is a `u8`, but it still differs from the printed
definition surface and should be documented or normalized.

## Definition And Pool Consistency Notes

`Dark Embrace` deserves a targeted cleanup pass. It is locally defined as cost 1
and `card_type_and_rarity` marks it Rare, while the Ironclad reward pool treats
it as Uncommon. The public baseline has cost 2 and rarity Rare.

The earlier audit pass also found non-Ironclad/status representational issues
such as curses being modeled as `CardType::Status`, but those are outside this
Ironclad-card-focused report except where they affect Ironclad runs through
curse rewards or events.

## Corrections To Prior Framing

The prior shorthand, "all Ironclad base cards are modeled; upgraded-card parity
has rough edges," is directionally right but understated. A sharper statement is:

All Ironclad base cards appear to have local definitions and combat dispatch.
However, upgraded Ironclad coverage is incomplete: many upgraded variants are
absent as content definitions, and several existing upgraded definitions disagree
with the public baseline.

One stale caveat from older docs should also be treated carefully: `Seeing Red`
is currently marked Exhaust in the local definition and routed through
`seeing_red_queue`, so older notes implying local discard behavior appear out of
date.

## Recommended Follow-Up Order

1. Add missing upgraded Ironclad definitions and `upgrade_content_id` mappings
   in small batches.
2. Fix high-confidence existing definition mismatches: `Twin Strike+`,
   `Pommel Strike+`, `Dual Wield`, `Dual Wield+`, `Sword Boomerang+`, and
   `Dark Embrace`.
3. Decide on an explicit representation for X-cost cards instead of storing
   printed X-cost as `0`.
4. Decide how to represent repeated-upgrade cards such as `Searing Blow`.
5. Reconcile `card_type_and_rarity`, reward-pool rarity, and public rarity for
   `Dark Embrace`.

## Verification Performed

- Inspected local card definitions and `ALL_CARDS` inventory.
- Inspected Ironclad reward-pool base-card entries.
- Inspected `play_card_queue` dispatch coverage.
- Compared local definitions against the `sts.gg` embedded STS1 card dataset.
- Requested and incorporated an independent sub-agent review.
- Did not run simulator tests because this change is documentation-only.
