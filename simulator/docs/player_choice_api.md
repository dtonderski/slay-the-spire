# Public Combat Choice API

Status: implemented in `sts_core` for the combat-only V1 slice.
Last updated: 2026-07-23.

This document describes the public legal-action half of
`fair_combat_api_design.md`. It does not define fair observations, tensors,
Python objects, full-run choices, or belief search.

## Boundary

There is still one authoritative action system:

```text
RunDecisionAction
        |
        | project visible references
        v
PlayerChoice
```

`legal_run_decision_actions` remains the source of truth for legality.
`player_choices` enumerates that boundary, converts internal IDs to public
decision-local slots, rejects ambiguous projections, and returns choices in a
canonical order determined only by public variant and slot fields.

The reverse operation, `resolve_player_choice`, re-enumerates the same
authoritative boundary and returns its existing `RunDecisionAction`. It does
not implement a second legality engine.

The implementation lives in
`simulator/crates/sts_core/src/run/player_choice.rs` and is re-exported by
`sts_core`.

## V1 Types

```text
DecisionRevision(u64)

PlayerChoiceSet
  schema_version
  decision_revision
  choices: [PlayerChoice]

PlayerChoiceRequest
  decision_revision
  choice: PlayerChoice
```

`PlayerChoice` covers every fair-stable combat decision currently exposed at
the unified run boundary:

| Public choice | Authoritative action |
|---|---|
| `PlayHandSlot { hand_slot, target_slot? }` | `CombatAction::PlayCard` |
| `EndTurn` | `CombatAction::EndTurn` |
| `UsePotionSlot { potion_slot, target_slot? }` | `RunAction::UsePotion` |
| `DiscardPotionSlot { potion_slot }` | `RunAction::DiscardPotion` |
| `ToggleVisibleCard { option_slot }` | hand/draw/discard/exhaust selection action |
| `ChooseVisibleOption { option_slot }` | combat card-reward choice |
| `ConfirmSelection` | active combat selection confirmation |
| `SkipSelection` | skippable combat card reward |

Slots are unsigned 16-bit decision-local references. Projection fails closed
if an authoritative collection cannot be represented. Internal `CardId` and
`MonsterId` values are never serialized by these types.

Potion discard is now included in the authoritative legal action enumeration;
the public layer projects it rather than synthesizing extra legality.

The V1 public projection intentionally omits Secret Weapon and Secret Technique
card-play commands. Their authoritative legality depends on draw-pile
composition, while this pure boundary has no public-knowledge contract for
unrevealed composition. The authoritative internal action list is unchanged;
this is a conservative public capability restriction, not a second legality
engine. See `design_fair_public_legal_action_visibility.md`.

## Revision Ownership

`DecisionRevision` is an explicit monotonic public counter. It is not computed
from a state hash and contains no seed, RNG counter, internal ID, or other
hidden-state fingerprint.

The eventual fair environment owns the counter and must follow this protocol:

1. Call `player_choices(run, current_revision)` and return that revision with
   the atomic fair decision.
2. Submit `PlayerChoiceRequest { decision_revision, choice }` to
   `resolve_player_choice(run, current_revision, request)`.
3. Apply the resolved authoritative action.
4. Advance the revision exactly once after the accepted decision.
5. Also advance or invalidate the revision if any privileged path replaces or
   mutates the authoritative state.

Revision comparison happens before authoritative state inspection. A request
with an old revision therefore always returns `StaleDecision`, regardless of
hidden state. The pure core mapping cannot detect callers that mutate state
without advancing their own counter; enforcing counter ownership belongs to
the stateful fair environment integration.

## Stable Public Errors

The public error surface contains only:

- `NotInCombat`
- `DecisionUnavailable`
- `StaleDecision`
- `InvalidChoice`

Internal validation failures, IDs, and mechanic-specific reasons collapse to
`DecisionUnavailable`. Unsupported or ambiguous internal projections also fail
closed with that error.

## Determinism and Non-Interference

The V1 tests establish that choice values, ordering, serialized bytes, and
public invalid-choice errors are unchanged by:

- hidden draw-pile permutations, including a Havoc top-card change;
- hidden draw-pile composition changes for Secret Weapon and Secret Technique;
- run and combat RNG seed/state changes;
- internal card and monster ID renumbering;
- hidden enemy-intent changes while Runic Dome is present.

Tests also cover visible-slot resolution, stale revisions, potion slots,
selection screens, combat reward choices, malformed-state error collapsing,
and side-effect-free enumeration.

## Deferred Integration

- moving the atomic `FairDecision` projection into `sts_core` itself (the
  current `sts_sim` facade combines the two pure Rust projections);
- extending the state-owning Python environment beyond combat and adding
  privileged trace/replay constructors for production workflows;
- PyTorch action descriptors and batching;
- non-combat run screens;
- particle or belief search.
- re-enabling hidden-dependent card plays after public draw-pile knowledge is
  represented in the atomic fair observation/history contract.
