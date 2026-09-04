# Project History

This is a curated record of major decisions, not a changelog. Git, tests, and
immutable traces hold implementation detail.

## Simulator and verification

The project began as both an attempt to build a strong Ironclad A20 Heart agent
and an experiment in whether LLM-assisted development could produce a faithful
Slay the Spire simulator. Rust was chosen over adopting `sts_lightspeed` because
its type system made iterative invariants easier to inspect and because prior
simulators could not serve as real-game authority.

The simulator therefore retained action queues, named RNG streams, snapshots,
and deterministic replay instead of simplifying mechanics for early RL. Real-
game CommunicationMod traces became the primary evidence.

Early development mixed simulator, search, verification, live tooling, UI, and
RL code under one umbrella. A workspace reorganization separated those owners.
The later leanification went further: speculative search/RL and the Rust live
application were deleted, leaving the core simulator, strict verifier, thin
Python binding, and collection bridge. Removed implementations and their design
documents remain recoverable from Git history.

## Trace-driven fidelity

Plausible mechanics repeatedly failed on queue order, delayed effects, hidden
RNG consumption, identity preservation, and importer semantics. Work shifted to
first-divergence diagnosis against immutable traces. Observations became expected
output only, never a source for simulator repair.

A major verifier failure temporarily violated that principle: replay generated
alternative transitions and selected whichever matched the post-state. At its
peak, dozens of fallback families made green traces prove only that one guess
matched. Removing all post-state candidate selection caused the corpus pass rate
to collapse, exposing honest simulator defects and collector races. The durable
rule is architectural: transition code must not see expected post-state.

CommunicationMod could also publish readiness during unresolved work. Boundary
schemas evolved to identify polls, interaction-ready, quiescent, and terminal
responses; fence command execution; expose queued end turns and effects; and
prevent overtaking transport states from completing the wrong action. Schema
versions are collection epochs, not forward-compatible payload revisions.

Some target behavior is genuinely non-seeded. Courier colored restock uses the
UI-advanced process-global `MathUtils.random`, and Secret Portal eligibility uses
target gameplay time. These became narrow typed external inputs captured before
the transition, never values inferred from observed results.

## Collection lessons

SlayTheData guidance was initially used to reach varied run states, but legal
mapping became brittle once simulator and source trajectories diverged. Fidelity
discovery moved to uniformly random concrete actions advertised by
CommunicationMod, with collection and verification kept separate.

A later audit found the discovery collector had accumulated exclusions for known
divergences and hangs, discarded accepted-command failures, and abandoned weak
combats. Those policies censored the distribution and were removed. Every
advertised gameplay action is now eligible and post-start failures remain
immutable evidence.

Collection acceleration also corrupted evidence when multiplied frame delta
changed gameplay action lifecycles and later the dungeon playtime clock. Those
cohorts were frozen outside the authoritative corpus. Visual acceleration,
action ticks, and gameplay clocks must remain separate.

## Agent direction

The deterministic combat search was useful for collection but was never the
final player. Early PUCT experiments on first-combat A0 roots produced weak
value contrast and a large experimental stack. That stack, the old fair Python
`RunEnv`, and associated artifact schemas were deleted rather than maintained as
premature infrastructure.

The durable interface is the Rust fair boundary: public observations and
public, decision-local choices derived from authoritative legality. Future
search or learning systems should be built as consumers of that boundary and
must earn complexity from a concrete experiment.

## Durable lessons

- Exact replay and data collection are different products and evidence.
- A verifier that sees the answer will eventually select behavior to match it.
- A green suite measures verifier honesty before simulator fidelity.
- Captures are immutable; invalid epochs are quarantined, not repaired.
- Explicit environmental inputs are narrow exceptions to seed determinism and
  must be captured before the affected transition.
- Fairness belongs at observation/action boundaries; full state remains valid
  for simulation and verification but not policy input.
- One current schema is preferable to compatibility machinery for disposable
  research artifacts.
- Large speculative subsystems and per-fix design documents age poorly. Keep
  code, tests, traces, and a small set of current contracts.

## Open questions

- What trace coverage is sufficient before large simulator-only training runs?
- Which run-level learning approach should be attempted first?
- What seed count and compute budget define the final A20 Heart claim?
