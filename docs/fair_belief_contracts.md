# Fair Belief Contracts

Status: crate-private Rust foundation slice; belief update, fair search, and the
fair facade remain deferred. This slice is an independently sampled
exchangeable-future approximation, not an exact combat-entry posterior.
Companion audit: [`fair_observation_hidden_state_audit.md`](fair_observation_hidden_state_audit.md).
Architecture: [`fair_belief_architecture.md`](fair_belief_architecture.md), Section 5.

Privileged full-state PUCT remains teacher-only. This document does not authorize POMCP,
training, promotion, or sealed/audit evidence use.

## 1. Authority layers

```text
FairDecision + PublicEvents
        -> PublicCombatKnowledge
        -> FairBelief
        -> belief-owned particle index
        -> materialize_combat_rollout(belief, particle_index)
        -> GeneratedCombatRollout
```

These types are crate-private until a fair facade can issue observation-bound start
tokens and resolve public choices to private IDs. External crates cannot construct
knowledge, materialize particles, or step generated rollouts.

The layers are intentionally not interchangeable:

- `PublicCombatKnowledge` contains fair observations plus the public action/event prefix.
- `FairBelief` adds a versioned prior, named belief RNG, and weighted hypotheses. It contains no
  simulator state. Particles are private. Debug output is redacted.
- A particle's hidden assignment contains independently sampled pile permutations, independently
  sampled combat RNG raw states, and independently sampled run-envelope seeds. It is not a public
  type, does not deserialize, and does not contain a `RunState`, snapshot, internal IDs, queues, or
  hashes.
- `GeneratedCombatRollout` is fresh authoritative state created only after sampling. It is valid
  internal planner authority, but it is not the real state and is not a full-run posterior sample.
  Stepping is refused once combat has terminated, and it never enters reward or map
  screens. Debug output does not print the private `RunState`.

There is no initializer whose input is `RunState`, `Snapshot`, snapshot JSON, true seed, state hash,
or true RNG. Any future API that adds one violates this contract.

## 2. Hard anti-hydration rules

1. Public values may be reconstructed from `PublicCombatKnowledge`.
2. Hidden values come only from a declared prior and a `HiddenHypothesis` sampled
   independently of truth. The current combat-RNG prior is an exchangeable-future
   approximation, not reconstructed post-entry stream state.
3. Observations may condition, filter, or reweight hypotheses. They never patch hidden fields in a
   survivor.
4. Materialization is deterministic and consumes no RNG. The same knowledge and hypothesis must
   produce byte-identical generated authority.
5. Every generated state validates and its `fair_combat_observation` equals the latest public
   observation exactly. Mismatch is an error, never a repair request.
6. Internal card/monster IDs are allocated deterministically from public entities. They do not come
   from the hypothesis or real game.
7. Belief RNG is distinct from game `StsRng`. Every draw has a named stream, call site, and counter.
8. Reported fair agents do not reconstruct game seeds/RNG streams from observed outcomes.
9. Neural inference receives fair observations, public choices/history, and explicitly fair-derived
   summaries only. It never receives hypotheses or generated authority.

## 3. Public history

A current `FairCombatObservation` is insufficient for arbitrary mid-combat reconstruction. Public
history can determine monster move history, known top-card placements, retained-hand state,
next-turn energy, and other gameplay facts omitted from the current projection.

The currently allowed root is the first player decision, constructed through
`PublicCombatKnowledge::at_first_player_decision` only when the caller holds an opaque
`PublicCombatStart` token bound to that observation. The token is not cloneable and is
consumed by the constructor. No production issuer exists yet: tests bind a token to a
fixture observation. Current pile shape and zero counters are explicitly not accepted as
proof.

Later roots will require a facade-validated complete typed public action/event/observation prefix or
checkpoint. No such constructor is exposed yet: all mid-combat roots, including snapshot plus
current observation, return `MissingPublicHistory` through `refuse_unproven_root`.

The Rust event vocabulary records public draws, plays, zone moves, shuffles, executed monster move
categories, and turn starts, but runtime emission and prefix validation are not integrated in this
slice. Mechanics needing history remain unsupported until those deterministic updates exist.

## 4. Priors and particles

Implemented prior: `a0_act1_simple_combat_exchangeable_v1`.

This is an independently sampled exchangeable-future approximation. It is **not** the
game's master-seed run initialization, and it is **not** the combat-entry joint process
after opening shuffle, HP rolls, and initial AI. Reconstructing those post-entry stream
states from public outcomes would be seed recovery, which remains out of protocol.
Calibration and search consumers must treat generated combat RNG as unrealized future
generators, not as the true first-decision streams.

Each weighted particle has positive integer weight and one private
`HiddenHypothesis` containing:

- authoritative storage-order index permutations for draw, discard, and exhaust public multisets;
- independently sampled run-envelope seeds that fill unreachable combat-shell fields; and
- independently sampled combat RNG raw states at counter zero, required to be pairwise distinct
  and nonzero.

The hypothesis type is not publicly constructible or deserializable. `FairBelief` keeps particles
private. Materialization accepts only a belief-owned particle index.

Without Frozen Eye, draw order is a Fisher-Yates uniform permutation sampled with rejection-bounded
belief draws. With Frozen Eye, the complete public top-to-bottom order determines draw storage order.
Frozen Eye never reveals discard or exhaust storage order; those are always sampled independently.

The materializer rejects malformed permutations and RNG (zero state, nonzero counter, or colliding
combat streams). It never normalizes or repairs them. Particle counts of zero or above the declared
bound return a typed error instead of allocating.

## 5. Fresh materialization

`materialize_combat_rollout(belief, particle_index)`:

1. loads the belief-owned hypothesis at that index;
2. validates schema, prior, supported scope, and public provenance;
3. allocates new card IDs from public hand/pile records and new monster IDs from public slots;
4. reconstructs supported public card/player/relic/monster state;
5. applies independently sampled pile permutations and independently sampled combat RNG;
6. builds a fresh canonical combat-only `RunState` envelope;
7. validates `CombatState` and `RunState`; and
8. requires exact equality between the generated fair observation and the latest public
   observation.

The canonical run envelope is not a belief about map, reward, event, shop, pool, or profile state.
Those fields are unreachable because this authority ends at combat termination. A future full-run
materializer needs explicit public knowledge and priors for every such field.

Cloning `GeneratedCombatRollout` inside search is allowed: it clones generated authority, not truth.
Stepping currently accepts internal `CombatAction` values and remains crate-private until the facade
resolves public choices.

## 6. Implemented support and typed refusals

The current successful subset is intentionally narrow:

- A0, Act 1;
- `waiting_for_player`;
- no active selection or queue-dependent boundary;
- no player powers or orbs;
- base A0 `Strike_R`, `Defend_R`, and `Bash` public card records without combat-local metadata;
- no occupied potion slots;
- stateless Burning Blood and Frozen Eye only; and
- opening Cultist roots, or the deterministic simple test monster.

Typed refusal applies to:

- Runic Dome/hidden intent: missing move-table posterior;
- other monsters: missing private-state reconstruction rule;
- other cards, dynamic costs/counters, powers, relic state, potions, or active selections;
- poisoned/incomplete public pile order;
- missing snapshot-only mid-combat history;
- malformed hypotheses; and
- observation mismatch after generation.

This is a materially useful construction proof, not fair search. No belief update, resampling,
policy, action selection, POMCP, or Python belief module exists in this slice.

## 7. Future belief update

A later update accepts only the current belief, the accepted public choice, emitted public events,
and the next fair observation.

For each hypothesis it may compute a source-backed likelihood, filter it, or sample directly from
the conditional posterior. It must distinguish:

- **sample depletion:** a finite particle set has no survivor although the mathematical posterior
  has support; direct posterior regeneration is allowed; and
- **model collapse:** the mathematical posterior has zero support; fail closed and fix the prior,
  event contract, or simulator.

Regeneration from the public posterior is not hydration. Restoring the true snapshot, copying true
hidden values, or widening observation equality is forbidden.

Resampling must be deterministic from named `belief.resample` draws and preserve serialized belief
RNG counters. No update/resampling API is exposed until these rules are implemented.

## 8. Future fair search

- Tree keys are public action-observation-event histories, never generated state hashes.
- One hypothesis is sampled/materialized per simulation.
- Statistics aggregate at public action descriptors.
- Every generated state at one public node must expose the same public legal descriptors.
  Disagreement is a projection/history bug, not a reason to union or intersect legal sets.
- The planner selects one action across the belief. Per-particle optimization followed by voting is
  strategy fusion and is forbidden.
- Search stops at combat termination until a run-level materializer exists.

## 9. Test invariants

The Rust tests require:

- deterministic belief and materialization for the same seed;
- hidden pile/RNG diversity with identical public projection;
- hypothesis-independent internal ID allocation;
- Frozen Eye fixing draw order while discard/exhaust remain sampled;
- independently sampled combat RNG and run-envelope seeds, documented as an
  exchangeable-future approximation rather than combat-entry reconstruction;
- exact generated projection and full state validation;
- typed failure for missing history, hidden intent, selections, unsupported public cards, and
  malformed pile/RNG hypotheses;
- positive weights enforced by a nonzero type;
- card-random continuity across combat-horizon stepping;
- observation-bound, non-cloneable first-decision tokens;
- redacted Debug output for beliefs and generated rollouts;
- typed refusal of zero or unbounded particle counts; and
- serialized belief and observation JSON matching a path-sensitive recursive schema,
  including event/choice/optional variants, with no public deserialization path for hidden
  hypotheses.

Existing fair-observation non-interference tests remain the projection gate.
