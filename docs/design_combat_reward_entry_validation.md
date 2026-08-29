# Combat Reward Entry Validation

Combat reward generation is an authoritative transition, not a general-purpose
screen constructor. Its production entry points must reject state that does not
prove a completed combat before consuming reward RNG or replacing run state.

The shared precondition is:

- the run passes `RunState::validate()`;
- the run is in `RunPhase::Combat` with a combat state;
- the combat phase is `CombatPhase::Won`;
- the player is alive; and
- no monster remains alive.

Normal, elite, and boss reward entry use the same check. Missing combat cannot
mean zero stolen gold, and stolen-gold accumulation must reject integer
overflow. Validation and overflow checks happen before mutation, while the
reward generators retain clone-then-commit behavior, so an error leaves run
state and RNG counters unchanged.

Tests that need a synthetic reward screen must construct an explicit won-combat
fixture. That keeps test convenience from weakening the production contract.
