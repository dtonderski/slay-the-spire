# Fair Observation and Choice API

The simulator owns full authoritative state. Fair consumers receive a public
projection and decision-local choices derived from authoritative legality.
This document describes the boundary, not a duplicate field/schema catalog.

## Ownership and entry points

[`sts_env`](../crates/sts_env/src/lib.rs) owns the fair environment:

- [`environment.rs`](../crates/sts_env/src/environment.rs): `FairEnvironment`,
  `FairDecision`, revision handling, and accepted transitions.
- [`combat_observation.rs`](../crates/sts_env/src/combat_observation.rs) and
  [`run_observation.rs`](../crates/sts_env/src/run_observation.rs): public
  projections and their schema constants.
- [`action.rs`](../crates/sts_env/src/action.rs): `PublicChoice`, requests,
  stable public errors, and mapping to core legal actions.

`FairEnvironment::decision()` returns observation and choices for the same
revision. `step()` resolves a current public choice to a core action and commits
only after the successor decision can be projected. There is no second legality
engine. Consult the source definitions for the current fields, choice variants,
and schema versions rather than maintaining another enumeration here.

Slots and candidate indices are decision-local references, not internal instance
IDs. `DecisionRevision` is an environment-owned monotonic token, not a seed or
state hash. It is used for stale-request rejection, not as a policy feature.
Public content keys identify visible card/relic/etc. types; they do not expose
private instance IDs or content-pool positions.

## Visibility and non-interference

Public observations include visible player/monster state, displayed intent,
hand slots, inspectable piles, visible relic/potion state, selection screens,
and allowlisted public counters. Run observations also cover noncombat screens.

They exclude hidden pile order, RNG, private AI state, internal instance IDs,
action queues, and unrevealed outcomes. Runic Dome intent is hidden. Draw order
is exposed only through explicit public-history knowledge or a public reveal
such as Frozen Eye. Discard and exhaust are public multisets, not internal
ordered vectors. See the [boundary audit](fair_observation_hidden_state_audit.md)
for detailed exclusions, tracking semantics, and known underexposure.

For states with the same public information and history but different hidden
information, projection, choice ordering, and public error behavior must not
reveal those differences. Projection and choice enumeration consume no RNG and
do not mutate state. The source tests exercise hidden-state perturbations and
public changes; these invariants are not real-game parity evidence.

## Python and policy consumers

[`sts_sim.State`](../python/sts_sim/__init__.py) wraps the fair environment.
`decision()` returns a typed public observation and its actions; `step()` accepts
an action from the current decision. The numeric batch interface transports
public data from the same environment. See the [Python API](python_api.md) for
usage and transport details and the [first-contact walkthrough](../../rl/examples/README.md)
for policy integration.

The public wrapper does not expose a full-state JSON/snapshot export. Constructor
seeds, synthetic specifications, revisions, and driver-side metadata still must
stay out of policy features: a fair API is not a sandbox around the caller.

**Cloning is not fair belief sampling.** A clone preserves the actual hidden
state. Searching live-state clones is privileged lookahead even when each
resulting observation is public. A fair planner needs hypotheses conditioned
only on public information/history and sampled independently of the actual
hidden state. The Python API does not provide a public-belief constructor.

Tensorization, learning, belief sampling, and search belong in consumers, not
`sts_core`. See the [RL README](../../rl/README.md) for the current training path.
