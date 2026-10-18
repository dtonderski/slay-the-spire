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

- [ ] **[RL] Establish whether shared choice-head pretraining improves run strength.**
  The actual run actor now loads supervised card/boss-relic preferences, but the
  tiny Act-3 comparison produced no positive rewards in either arm. Test a longer,
  multi-seed comparison with usable reward signal and account for offline compute;
  keep full legal sets, fixed held-outs and matched frozen combat. Separately
  investigate exact prefix decks, boss timing and audited Skip/Bowl legality before
  supervising richer state or cross-family choices. Partial logged history did not
  improve the original pilot overall. See the
  [implemented scope and measurements](rl/pretraining/README.md#shared-head-transfer-into-the-real-run-actor).

- [ ] **[Simulator/RL] Close the historical map-version gap before route supervision.**
  The 2022 A0 full-graph bytecode differential check passed, but the 2020 dataset
  still has only seed/recorded-route evidence. Obtain the historical JAR or full
  map captures, audit that version's initialization, and separately implement
  and validate A20 generation. Route symbols also leave coordinate ambiguity;
  do not turn research-only inferred paths into observed labels or repair a
  simulator from run logs. See the [map evidence](rl/pretraining/README.md#original-game-bytecode-differential-check-2022-a0)
  and [historical confidence limits](rl/pretraining/README.md#seed-based-maps).

- [ ] **[Simulator] Close remaining A0 rollout failure classes.** Investigate
  advertised Courier card purchases rejected as invalid, Prismatic Shard purchases
  with unavailable successors, and Headbutt selections that leave won combat with
  no legal next action. Preserve the complete legal action set and use typed
  external inputs where target mechanics require them. These simulator-only
  reproductions do not establish real-game behavior; require source-backed fixes
  and normal regression/corpus verification. See the
  [run-training limitations](rl/run_training/README.md#environment-limitations)
  and [RNG research](simulator/docs/research.md).

- [ ] **[RL] Learn calibrated macro values and HP-conditioned choices.** The first
  synthetic campfire curriculum and simple explicit-health paths did not improve
  held-out clearing. Test root-level value targets/weighting and explicit
  state–action-family interactions; distinguish representational failure from
  sparse positive experience and flat candidate-family exploration. Keep all
  legal candidates and separate natural-start evaluation. See the
  [settled experiment](docs/project_history.md#run-level-health-conditioning-and-campfire-pilot-october-2026)
  and [curriculum workflow](rl/run_training/README.md#synthetic-pre-boss-campfire-experiment).

- [ ] **[RL] Reduce remaining run-collection CPU work.** Profile typed observation
  projection and public feature assembly under the batched collector. Before
  adopting native numeric batch stepping, require per-state settlement/error
  attribution: the current multi-state transition API is not atomic. Preserve
  immutable journals, complete candidates, independent RNGs and whole-batch
  quarantine. Measure end-to-end gains at unchanged learning batch sizes; do not
  infer them from isolated inference speedups. See the
  [batched workflow](rl/run_training/README.md#batched-execution-and-throughput)
  and [numeric transport](simulator/docs/python_api.md).

- [ ] **[RL] Better map representation — deferred.** Is the position-dependent
  hashed map encoding limiting route decisions? First compare explicit public-map
  features (distance to rest, remaining/unavoidable elites and branching), then
  consider a small graph encoder with shared room features and actual edges.
  Condition destination scores on player/deck context and retain all legal
  candidates. Use only visible map information, never hidden encounters or
  simulator futures. Require held-out clear-rate and route-behavior evidence
  before adopting complexity. See the [current encoder](rl/run_training/model.py)
  and [historical investigation](rl/docs/run_level_training.md).
