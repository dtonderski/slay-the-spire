# Authoritative Monster Max HP

## Problem

Monster self-healing normally caps at the rolled `MonsterState::max_hp`.
However, if that field is nonpositive, the helper reconstructs a cap from the
static content definition. Unknown content is silently treated as
`FIXED_SIMPLE_MONSTER`.

That repair is both unnecessary and incorrect at an authoritative transition:
combat validation already requires known monster content and positive max HP,
and a static definition cannot recover a lost per-encounter HP roll. The repair
can therefore convert malformed state into plausible but divergent gameplay.

## Decision

- `MonsterState::max_hp` is the sole healing cap after encounter construction.
- Self-heal never reconstructs max HP from content or ascension.
- Nonpositive max HP remains an `InvalidState` error at the existing combat
  validation boundary, before an action can execute.
- Remove the public definition-based reconstruction helper and its
  `FIXED_SIMPLE_MONSTER` substitution.

Explicit fixtures must set valid max HP. Snapshot/import migration may reject
an old state whose rolled max HP was lost; it must not invent a replacement.

## Verification

Regression coverage must prove that a nonpositive monster max HP fails before
action execution without mutating the source state, while existing self-heal,
workspace, corpus, deterministic replay, and snapshot gates remain unchanged.
