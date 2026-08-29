# Typed combat decision binding

## Problem

The seed-start verifier previously rebuilt combat selection routing with a
sequence of independent booleans for card rewards, hand selection, discard
selection, and exhaust selection. Draw-pile selection was projected but had no
command binding. If imported or divergent state exposed multiple selections,
the first branch silently won. A separate hand-selection refresh flag was
derived from observed pre/post screen types even though core state already
owned the decision.

## Decision invariant

Before binding a combat decision command, the verifier classifies simulator
state into exactly one of:

- combat card reward;
- hand selection;
- draw-pile selection;
- discard-pile selection;
- exhaust-pile selection.

No active decision returns ordinary combat routing. More than one active
decision fails at `invalid_combat_decision_state`; no branch priority is a
repair mechanism. The classifier reads only authoritative `CombatState`.

## Command binding

`CHOOSE <index>` maps to the matching typed `RunAction` for all five decision
families. `CONFIRM` maps to the four pile-selection confirmations, while
`SKIP` is accepted only for combat card rewards. Any other command fails at
`unsupported_combat_decision_command`. The core transition result alone drives
the simulated projection.

## Regression contract

Focused coverage pins draw-selection `CHOOSE` and `CONFIRM` binding and requires
a synthetic state with simultaneous draw and discard selections to fail closed.
Permanent and fidelity replay protect existing card-reward, hand, discard, and
exhaust selection sequences.
