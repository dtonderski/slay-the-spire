# Shared backlog

Repository-wide queued work and investigation ideas. These are not an ordered
roadmap or promises. [Project knowledge](docs/README.md) routes to canonical
contracts and workflows; this file does not duplicate their implementation guides.
Keep one entry per question, link its owning subsystem, and remove resolved items.
Git holds completion history. Session logs, local artifact paths and benchmark
transcripts belong with experiment artifacts, not in this backlog.

## Queued work

No additional implementation is queued here. The experimental run-training
baseline and its reward/behavior metrics are documented in the
[run-training guide](rl/run_training/README.md).

## Ideas and investigations

- [ ] **[Simulator] Close remaining A0 rollout failure classes.** Investigate
  advertised Courier card purchases rejected as invalid, Prismatic Shard purchases
  with unavailable successors, and Headbutt selections that leave won combat with
  no legal next action. Preserve the complete legal action set and use typed
  external inputs where target mechanics require them. These simulator-only
  reproductions do not establish real-game behavior; require source-backed fixes
  and normal regression/corpus verification. See the
  [run-training limitations](rl/run_training/README.md#environment-limitations)
  and [RNG research](simulator/docs/research.md).

- [ ] **[RL] Batch run collection and learning.** Investigate concurrent independent
  episodes with batched frozen-combat inference, batched macro forward/backward,
  and numeric combat stepping between typed macro boundaries. Compare end-to-end
  throughput, not just model microbenchmarks; small serial GPU calls can lose to
  CPU inference. Preserve complete legal candidates, per-run RNG ownership,
  quarantine semantics and loss weighting. Establish equivalence and explicitly
  version changed protocols. See the
  [run-training guide](rl/run_training/README.md) and
  [numeric transport](simulator/docs/python_api.md).

- [ ] **[RL] Better map representation — deferred.** Is the position-dependent
  hashed map encoding limiting route decisions? First compare explicit public-map
  features (distance to rest, remaining/unavoidable elites and branching), then
  consider a small graph encoder with shared room features and actual edges.
  Condition destination scores on player/deck context and retain all legal
  candidates. Use only visible map information, never hidden encounters or
  simulator futures. Require held-out clear-rate and route-behavior evidence
  before adopting complexity. See the [current encoder](rl/run_training/model.py)
  and [historical investigation](rl/docs/run_level_training.md).
