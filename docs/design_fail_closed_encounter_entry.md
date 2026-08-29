# Fail-closed encounter entry

## Problem

Production encounter entry currently converts missing encounter generation into
an empty spawn list, retains `CombatState::initial_fixture()`, maps unknown game
monster IDs to Cultist, substitutes Bronze Automaton for an unknown Act 2 boss,
and substitutes Hexaghost for unsupported acts. Each path produces a plausible
combat instead of exposing unsupported or invalid state.

## Contract

Encounter-key and game-monster-ID lookup are fallible. Normal, elite, and boss
combat construction return `SimResult<CombatState>` and reject missing or empty
spawn generation, unknown monster content, and unsupported acts. Map-node and
Secret Portal transitions propagate those errors; because they operate on a
cloned `RunState`, a failed entry does not return a partially mutated run.

Known Act 1 boss fixtures and explicit test fixtures remain valid. This change
removes only implicit production substitution. Internally generated random
gremlin names remain a closed, asserted table until monster action execution is
itself converted to a fallible boundary.

## Evidence

Core regressions cover an unknown encounter key, an unknown spawned monster,
an unsupported boss act, and the public lookup functions. Existing map,
snapshot, workspace, and permanent replay suites protect supported encounters
and deterministic behavior.
