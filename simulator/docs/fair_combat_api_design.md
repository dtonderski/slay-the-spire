# Fair Combat API Design

Status: implementation contract for the first fair combat API slice.
Last updated: 2026-07-23.

This document narrows the active work that precedes combat RL. It covers the
symbolic public observation and public player-choice boundary together. It does
not specify PyTorch tensors, belief particles, POMCP, or a learned model.

The authoritative simulator remains unchanged: it owns full state, exact legal
actions, and transitions. Fairness is enforced by a pure projection and a
leakage-proof public representation of the same legal choices.

## Decisions

1. There is one game action, not separate "fair" and "omniscient" actions.
   `CombatAction` is the authoritative internal command. `PlayerChoice` is the
   public, stable way to describe what the player can select.
2. `FairCombatObservation` and `PlayerChoice` are symbolic Rust data. Tensor
   extraction belongs outside simulator mechanics and is deliberately deferred.
3. The compiled Python package will ultimately be one module (working public
   name `sts_sim`) with a fair decision API and explicitly privileged
   omniscient/debug APIs. Do not create a separate `sts_fair` crate.
4. The first implementation is combat-only. Full-run screens are a later slice.
5. Belief and particle planning are deferred. The same fair boundary will be
   reused when a singleton privileged search root is replaced by a belief over
   hidden roots.

## Public Decision Contract

The public boundary is a single atomic decision:

```text
FairDecision
  schema_version
  decision_revision
  observation: FairCombatObservation
  choices: [PlayerChoice]
```

`decision_revision` prevents a choice from an old state from being resolved
against a new state. It must not encode a seed, internal ID, state hash, or
other hidden information.

Rust constructs a decision by:

```text
authoritative CombatState
        |-- project public fields --> FairCombatObservation
        `-- enumerate authoritative legal actions
              --> map to PlayerChoice values
              --> canonicalize by public fields
```

When the caller submits a choice, Rust resolves its visible references against
the current authoritative state and applies the corresponding internal action.
Internal `CardId`, `MonsterId`, content-pool position, and generation provenance
never cross the fair boundary.

## FairCombatObservation V1

Indicative symbolic schema; final Rust names may follow existing core naming:

```text
FairCombatObservation
  schema_version
  turn
  decision_phase
  player
    hp, max_hp, block, energy
    visible powers
  hand
    visible slot
    card content key, upgrade level, visible cost
    visible dynamic card fields
  draw_pile
    count
    known unordered card multiset
    publicly known ordered prefix, if any
  discard_pile
    count, unordered visible contents
  exhaust_pile
    count, unordered visible contents
  monsters
    visible slot, kind, hp, max_hp, block
    visible powers
    visible intent category, damage, hit count
    alive / escaped / minion / targetable flags
  relics
    identity and explicitly allowed public counter/state
  potion_slots
    visible slot and potion identity, or empty
  current visible selection, if any
  public counters needed at the decision boundary
```

### Cards and piles

- Hand slots are public references used to select a card. They are not a
  semantic ordering for the future neural model. Permuting hand records and
  the corresponding choice references must permute policy outputs and leave
  value unchanged.
- Without an explicit visibility rule, the draw pile is an unordered multiset.
  Frozen Eye or a publicly observed placement may expose a full order or known
  prefix. Unknown positions are absent, not filled from simulator order.
- Discard and exhaust are public multisets. Internal order is not exposed merely
  because the simulator stores a `Vec`.
- Public card records contain no instance ID. Visible slots provide temporary
  decision-local references. Duplicate cards with identical visible state are
  intentionally indistinguishable to the model.

### Powers, statuses, relics, and intent

- Status cards such as Burn and Wound are ordinary public card records in their
  visible piles. Weak, Vulnerable, Strength, and similar conditions are public
  power records attached to their visible owner.
- A generic internal counter is not public by default. Relic and power fields
  require an explicit allowlist backed by real UI visibility or derivability
  from public history.
- Enemy intent includes only what the UI displays. When intent is hidden, the
  observation contains an explicit hidden/unknown marker and omits the real
  category, damage, and hit count.
- Private AI state, hidden move rolls, RNG streams, queue internals, and future
  outcomes are never projected.

### Public history

V1 may project only information already represented in current public state.
The type must remain extensible to a deterministic public-history summary or
event stream. History-derived information is fair only when it can be computed
without inspecting hidden state.

## PlayerChoice V1

Names are indicative. The descriptor family should cover the combat choices
already supported by the authoritative decision boundary:

```text
PlayerChoice
  PlayHandSlot { hand_slot, target_slot? }
  EndTurn
  UsePotionSlot { potion_slot, target_slot? }
  DiscardPotionSlot { potion_slot }
  ToggleVisibleCard { option_slot }
  ChooseVisibleOption { option_slot }
  ConfirmSelection
  CancelSelection                 # only where the game exposes it
```

Do not duplicate legality rules. Projection starts from the existing legal
internal action list, converts IDs to visible decision-local slots, removes any
public duplicates, and sorts using only serialized public descriptor fields.

Resolution performs the reverse mapping against the same decision revision.
Errors are public and stable: they must not reveal whether failure came from a
hidden identity, hidden target, internal ID, or private simulator condition.

## Non-Interference Invariant

For authoritative states `s1` and `s2` with equal public history and differences
confined to hidden state:

```text
fair_observation(s1) == fair_observation(s2)
player_choices(s1)   == player_choices(s2)
public errors(s1)    == public errors(s2)
```

Equality includes serialized bytes, list length, ordering, optional-field
presence, and error category. Projection and choice enumeration consume no RNG
and do not mutate simulator state.

Required tests for the first slice:

- hidden draw-pile permutations do not change observation or choices;
- RNG seed/counter changes do not change observation or choices;
- internal card/monster ID renumbering does not affect public serialization or
  ordering;
- hidden intent does not leak through observation, choice shape, or errors;
- publicly revealed order or intent does change the observation at the proper
  reveal boundary;
- visible slot projection resolves to the intended authoritative action;
- stale or invalid visible references fail with the same public error across
  hidden-equivalent states;
- projection and enumeration are deterministic and side-effect free.

## Rust and Python Placement

The symbolic types and pure projection/mapping logic should live near the
authoritative Rust combat API so they can be tested without Python. They must
not introduce tensor or training dependencies into `sts_core`.

The existing extension is currently named `sts_omni`. A later integration slice
will expose the fair decision API and privileged APIs from one compiled module;
renaming/repackaging is not required for the two initial Rust worktrees. Fair
Python objects must not offer snapshot, restore, raw state JSON, hashes, RNG
details, or debug logs. Privileged objects must remain visibly named as such.

## Deferred Work

- PyTorch observation and action tensor schemas, vocabularies, padding, and
  normalization;
- public-history/recurrent input contract beyond the V1 extension point;
- full-run observations and choices;
- particle beliefs, POMCP, or latent dynamics;
- adversarial statistical leak probes beyond the first deterministic and
  property-test suite.
