# Fair Observation and Choice API

The simulator owns full authoritative state. Fair consumers receive a pure
public projection and decision-local choices derived from the same authoritative
legality rules.

## Rust API

Implementation:

- `simulator/crates/sts_core/src/combat/fair_observation.rs`
- `simulator/crates/sts_core/src/run/fair_observation.rs`
- `simulator/crates/sts_core/src/run/player_choice.rs`

Current schemas are combat observation V2, run observation V1, and player
choice V2. Producers emit only current schemas.

`fair_combat_observation` projects combat state. `fair_run_observation` wraps
combat and map/event/reward/treasure/rest/shop/grid/complete screens.
`player_choices` maps authoritative legal actions to public descriptors;
`resolve_player_choice` maps a descriptor back to the existing
`RunDecisionAction`. There is no second legality engine.

Public choices are:

- `PlayHandSlot { hand_slot, target_slot? }`
- `EndTurn`
- `UsePotionSlot { potion_slot, target_slot? }`
- `DiscardPotionSlot { potion_slot }`
- `ToggleVisibleCard { option_slot }`
- `ChooseVisibleOption { option_slot }`
- `ConfirmSelection`
- `SkipSelection`
- `Proceed`

Slots are unsigned, decision-local references. Internal card, monster, and
content IDs never cross this boundary. `DecisionRevision` is a monotonic public
counter owned by the stateful caller; it is not derived from state, seed, RNG,
or hashes. Stale and invalid requests fail through the stable public errors
`NotInCombat`, `DecisionUnavailable`, `StaleDecision`, and `InvalidChoice`.

## Visibility

Public observations include visible player and monster state, displayed intent,
hand slots, inspectable pile contents, visible relic/potion state, active
selection screens, and explicitly allowlisted public counters.

They exclude:

- hidden pile order unless a public effect reveals it;
- RNG state and future random outcomes;
- internal IDs and content-pool positions;
- private monster AI state and hidden intent;
- action queues, limbo, and pending-effect scaffolding;
- unrevealed rewards, rooms, shops, and events.

Discard and exhaust are public multisets, not ordered internal vectors. Draw
order is hidden without a visibility rule such as Frozen Eye. Runic Dome intent
is emitted as hidden without category, damage, or hit count. Unknown public
content fails closed rather than leaking an internal identity.

## Non-interference

For states differing only in hidden information:

```text
fair_observation(s1) == fair_observation(s2)
player_choices(s1)   == player_choices(s2)
public_errors(s1)    == public_errors(s2)
```

Equality includes serialized bytes, ordering, optional fields, and error class.
Projection and choice enumeration consume no RNG and do not mutate state.
Tests cover hidden pile permutations, RNG changes, ID renumbering, Runic Dome,
visible-slot resolution, stale revisions, canonical ordering, and public
changes that must alter the projection.

## Python boundary

The current Python binding exposes `State` and `Action`; see the
[Python API documentation](../../simulator/docs/python_api.md).
Its combat actions use the Rust public-choice mapping. `State.to_json()` is a
privileged full-state serialization, not a fair observation. Python does not
currently expose `fair_run_observation`, so policy code must not treat raw state
JSON as fair input.

Tensorization, public-history encoding, belief sampling, and search belong
outside `sts_core`.
