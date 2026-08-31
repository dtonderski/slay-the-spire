# Combat Agent Phase 2 Plan

Status: beam-cloning training vertical slice implemented; naive synchronous privileged PUCT distillation (Record V3 / manifest V6) exists; Stage 6 batching/virtual-loss/performance gates remain open.
Last updated: 2026-08-31.

This document settles the architecture decisions that precede writing the
Expert Iteration loop, and stages the work behind measurable gates. It is
scoped to combat. Run-level learning, belief/particle search, and the A20H
evaluation protocol are out of scope.

The first training-ready slice now implements deterministic legal simulator
roots, a public-decision replanning teacher that reuses the extracted `sts_search`
beam core, symbolic Record V2 datasets, masked terminal values, exact CPU
checkpoint/resume, optional offline W&B scalar/provenance tracking, and
development evaluation. Training can clone beam V2 labels or privileged PUCT V3
visit/root-mean labels; W&B metadata follows manifest V6 rather than an
unchecked teacher-name literal. Truncated V3 PUCT rollouts keep root-mean
value targets present and unmasked. Offline W&B
resume starts a new run segment; `target/wandb` is
removed by `cargo clean`; source/`uv.lock` changes reject old checkpoints.
It intentionally does not claim batched PUCT, candidate promotion, or
trace-derived root extraction. The native episode payload currently reports the full HP/max-HP/gold,
potion, status, decision/turn, and truncation contract; card/relic counter deltas
remain an empty versioned field until core owns their terminal capture.

Companion contracts, unchanged by this plan except where a decision below says
otherwise:

- [`combat_rl_architecture.md`](combat_rl_architecture.md) — model, encoder,
  training-loop, and objective design.
- [`fair_combat_api_design.md`](fair_combat_api_design.md) — symbolic fair
  observation and public choice boundary.
- [`combat_turn_search_design.md`](combat_turn_search_design.md) and
  [`combat_search_benchmark_2026-07.md`](combat_search_benchmark_2026-07.md) —
  the incumbent planner and its frozen benchmark protocol.

## Measured Baseline

All figures from a release build on the combat fixture, 2026-08-27, RTX 5080
Laptop. The fixture is a small state (13 entity tokens, one-card draw pile), so
these are optimistic bounds, not late-act costs. Recorded here so later work has
something to compare against.

| Path | Per call | Rate |
|---|---|---|
| `RunState` clone (native) | 0.4 us | 2,800,000/s |
| Rust beam search, internal transitions | ~7 us | ~135,000/s |
| `native.legal_actions()` | 33.4 us | 30,000/s |
| `native.observation_json()` | 41.6 us | 24,000/s |
| native clone + legal_actions + step | 50.4 us | 20,000/s |
| `snapshot_hash` / `snapshot_json` | 5.9 / 3.3 us | — |
| Python `decision()` | 121.7 us | 8,200/s |
| Python clone + decision + step | 274.1 us | 3,600/s |
| `tensorize_combat` | 165.8 us | 6,000/s |
| `collate_combat_tensors((one,))` | 92.4 us | 10,800/s |
| Model forward, batch 1, 24 CPU threads | 181.8 ms | 6/s |
| Model forward, batch 1, 1 CPU thread | 0.68 ms | 1,470/s |

The tiny policy/value network has 317,954 parameters.

Two facts drive most of this plan. A full Python clone/decision/step plus
singleton tensorization and collation costs roughly 530 us against roughly 7 us
of internal simulation, a factor of about 75. The components are reported
separately above; the singleton numbers do not predict batched throughput. And
the default torch CPU thread pool makes the network 267x slower than
single-threaded on batches this small, because a 13-token transformer cannot
absorb 24 threads.

## Current Boundaries

1. **Trace-derived roots remain blocked.** `combat-research freeze` still needs
   its duplicate-metadata fix and verifier-owned streaming root capture. Until
   then, trace roots are evaluation anchors in the design, not accepted training
   artifacts produced by the new CLI.
2. **Beam cloning uses one extracted replanning teacher.** Offline labeling
   calls the `sts_search` beam core with a deterministic transition budget, no
   deadline, and no warm suffix at every public decision. This is not the live
   automation execution policy. The repository-unused legacy PyO3
   objective-string search surface was deleted in this extraction under the
   approved no-shim policy when the incumbent planner moved into `sts_search`.
3. **Supervised training is ready; Expert Iteration is not.** The implemented
   CLI generates manifest-bound records, fits training-only vocabularies,
   performs masked policy/value optimization, resumes exactly at batch
   boundaries, and evaluates development records. Static imitation evaluation
   (`sts-combat-evaluate`) scores labeled public rows; matched gameplay rollout
   (`sts-combat-rollout`) compares random, greedy-network, and beam episodes on
   independently restored roots and does not promote a candidate. Naive
   synchronous privileged PUCT now searches one public decision at a time with
   batch size 1 and no virtual loss. A replay buffer, fair batched leaf
   evaluation, and sealed candidate promotion remain unimplemented.
4. **Training roots are hybrid by contract.** Legally generated simulator roots
   are the scalable training source. Immutable real-trace roots remain fixed
   development/sealed/audit anchors once authoritative streaming extraction is
   complete. Arbitrary raw-state randomization is not an accepted data source.

## Decisions

### D1. The fair boundary must represent post-combat control, and combat episodes stop first

`RunEnv.step()` must classify a combat terminal before it requests another
combat-model decision. `Proceed` is post-episode run control and is never a
combat policy target. Directly observing or restoring a won/lost combat still
needs a valid public run decision, so add `PlayerChoice::Proceed` as a deliberate
**PlayerChoice V2** change rather than hiding it or silently reusing another
choice family.

The V2 change updates the Rust schema and kind inventory, PyO3 mappings, Python
literal/action descriptors, tensor action specification, field-coverage and
cross-language schema tests, documentation, and checkpoint compatibility.
Stored V1 decisions remain readable; new producers emit V2. Do **not** relax the
catch-all to `Ok(None)`: every action reachable while `RunPhase::Combat` gets an
explicit projection or a documented fail-closed omission.

The gate combines exhaustive mapping review with 1,000 committed deterministic
seed/policy episodes, including lethal card play, lethal End Turn, restored
won/lost states, escape, and post-combat Proceed. Finite randomized coverage is
a regression probe, not a proof of completeness. The 433-trace gate supplies
additional reachable-state coverage.

### D2. Project Secret Weapon and Secret Technique

[`design_fair_public_legal_action_visibility.md`](design_fair_public_legal_action_visibility.md)
omits these two card plays because "the pure V1 public boundary does not carry a
public-knowledge model for unrevealed draw-pile composition." That premise no
longer holds. The implemented projection exposes `draw_pile.cards` as a
canonical multiset of the full draw-pile contents, hiding only order via an
empty `known_order`. Both predicates — Secret Weapon requires an Attack in the
draw pile, Secret Technique requires a Skill — are order-invariant functions of
that public multiset.

Projecting them therefore leaks nothing and preserves the non-interference
invariant: permuting hidden draw order cannot change either predicate. Leaving
them omitted instead imposes a permanent, silent strength ceiling on every fair
policy. Update the visibility design doc alongside the change, and extend the
hidden-permutation property test to cover both cards.

### D3. `sts_core` owns the exhaustive combat episode contract

The episode is defined and terminated in Rust, not reconstructed in Python.
`sts_core` owns a versioned, serializable episode specification, terminal
classifier, and root/terminal outcome builder:

- `Ongoing`: the accepted transition remains inside the combat episode.
- `Won` or `Lost`: the authoritative combat reaches that terminal phase, or the
  accepted transition exits combat through the corresponding authoritative
  victory/defeat path. Classification uses the transition and pre/post state, so
  it does not require a surviving terminal `CombatState`.
- `Escaped`: an accepted combat action, such as Smoke Bomb, exits combat without
  a win/loss through the authoritative escape path.
- `Truncated`: `combat_episode_v1` allows at most 512 accepted public decisions
  and 100 episode-local player turns. The root's current player turn counts as
  turn one; selection overlays do not start turns; the counter increments only
  when an accepted transition first reaches the next ordinary player-input turn.
  The episode truncates before exposing a decision on turn 101. After every
  accepted decision, classify won/lost/escaped first; therefore a terminal 512th
  decision is terminal, not truncated. Only a still-ongoing episode is truncated
  after its 512th decision. Both counters and the triggering bound are stored.

Rust produces the full outcome vector at that boundary: status, terminal HP and
max HP, HP/max-HP/gold deltas against the root, remaining potion identities and
slots, and combat-changed relic/card counters. `CombatOutcome` in
`python/sts_sim/rl/records.py` becomes a reader of that payload. Python's
Smoke-Bomb inference is removed after the native contract exists.

Current vertical-slice note: the native PyO3 bridge classifies the transition
from authoritative pre/post `RunState` values and emits every field above except
combat-changed relic/card counters, whose versioned array is currently empty.
Moving that contract into `sts_core` and populating those counter deltas remain
part of the Stage 2 production gate; the implemented slice does not claim them.

Truncated episodes are not losses. Version the symbolic training-record contract
so `target_value` may be absent with an explicit value mask; their search-policy
targets may be retained, but they do not contribute to value loss. Dataset
collation and loss tests must prove the mask is honored. Storing the full vector
while search consumes one scalar remains the durable contract.

### D4. One teacher, in a new `sts_search` crate

Create `crates/sts_search`, depending only on `sts_core`. Move the planner core
out of `sts_live::automation`: the node, recommendation, and outcome types, the
scoring and dominance functions, the action-depth and complete-turn beams, the
inner turn expansion, width pruning with first-action diversity, warm-suffix
validation, the optional fingerprint dedup, and the planner-action label codec.

Constraints on the move:

- **Behavior must not change.** The July 2026 benchmark conclusions are only
  valid against the incumbent objective and budget. The keepers —
  turn-boundary replanning with a warm suffix, lexicographic HP-before-potions
  dominance, and a budget-neutral complete-turn fallback used only when the
  primary search is nonterminal — move unchanged. Before extraction, regenerate
  a golden report on every development root. After extraction, compare every
  deterministic result field, first action, full principal variation, outcome,
  node/generated/expanded/pruned counts, terminal statistics, and replay error
  exactly. `elapsed_ms` is measured separately and excluded from equality; the
  serialized field inventory and names get their own compatibility test.
- **Split the config, do not move it.** `AutomationConfig` is a serialized live
  wire type embedded in session snapshots and the automation journal. Define
  `sts_search::SearchConfig` with the fields search actually reads and give
  `sts_live` an `AutomationConfig::search_config` adapter. Serde field names in
  `AutomationConfig` must not change.
- **Warm start takes search types.** Accept `&[PlannerAction]` rather than
  `&[AutomationPlannedAction]`; let `sts_live` decode its cached labels.
- **Keep the label codec with the search.** First-action diversity currently
  keys on the formatted label string, and the plan-suffix cache round-trips
  through it, so the codec and pruner move together unchanged. Value-keyed
  diversity is a later semantic change with its own equivalence or benchmark
  gate; it is not part of the extraction attestation. If pursued, use an
  explicit `PlannerActionKey` unless changing core action trait contracts is
  separately justified.
- **`BenchmarkSearchResult` field names are a data contract.** Historical
  reports parse that JSON; keep it identical.
- **Deadlines stay injected.** The only nondeterminism in the planner is
  `Instant::now()`. Keep the deadline an `Option<Instant>` parameter so budgeted
  replay stays bit-reproducible, and keep passing `None` from benchmarks.

The training pipeline uses only this teacher. The legacy `py_sts`
objective-string search was repository-unused and was deleted in this extraction
under the approved no-shim policy; no duplicate compatibility adapter remains.

### D5. Search runs in Rust; a generic fair evaluator batches across the FFI

PUCT lives in `sts_search`, over authoritative `RunState` transitions, generic
over a fair leaf-evaluator trait. PyO3 and any future Rust encoder adapter live
in `py_sts`, not `sts_search` or `sts_core`. The current vertical slice is
synchronous batch-size-1 PUCT: it reports visits, root-mean value, transitions,
simulations, leaf evaluations, and a `stop_reason` of `simulation_budget` or
`transition_budget`. `c_puct` must be finite and positive. Search always runs to
one of those bounds, carries public episode-root HP/gold baselines across
replanning, and invokes the fair leaf callback synchronously while holding the
Python GIL. Terminal revisits use standard MCTS backup, so visit targets can
overweight short terminal lines. `fair_leaf_batch_v1` is intentionally not
extensible. It does not yet implement virtual loss, batched leaf evaluation, or
the Stage 6 performance gate. Duplicate resolved authoritative actions and
missing leaf-response schemas fail closed.

```text
Rust PUCT
  -> traverse edges keyed by canonical serialized PlayerChoice
  -> resolve each choice against that node's authoritative state
  -> reserve transition budget, apply the authoritative transition
  -> collect up to B pending leaves (virtual loss on the paths)
  -> one evaluator call: [(detached fair observation, detached choices)] x B
       Python: tensorize, collate, one forward pass
  -> returns [(priors aligned to choices, finite value)] x B
  -> unwind virtual loss, expand, and back up in canonical order
```

Expansion uses exactly `player_choices`; it never expands hidden internal actions
and evaluates only a public subset. Projection and resolution must remain
bijective or fail closed. The evaluator never receives `RunEnv`, `RunState`,
a snapshot/hash, RNG fields, private action handles, or authoritative IDs.

Every successfully applied authoritative edge consumes one shared transition
budget unit. Root pre-evaluation consumes zero transitions and is not a
simulation. Traversals are collected in canonical order; before each edge they
atomically reserve one remaining unit, and no traversal starts or continues an
edge without it. A terminal traversal backs up immediately. Traversals reaching
the same full authoritative leaf pending in the current batch share one
evaluation but retain separate paths; each becomes one completed simulation only
when that finite result is backed up. The final nonempty partial batch is
evaluated and backed up even after the transition budget is exhausted.

Evaluator failure or malformed output unwinds every virtual loss, performs no
backup for affected traversals, leaves already applied transitions charged, and
fails the search rather than returning a partial recommendation. Completed
simulations count successful terminal or evaluated path backups, not unique leaf
evaluations. Backup order is original traversal-collection order regardless of
evaluator return scheduling. Reports contain transitions, completed simulations,
unique evaluations, duplicate attachments, and aborted traversals.

Deterministic CPU tests fix checkpoint, evaluator batch size, backend, budget,
canonical choice tie-breaking, pending-leaf and backup order, duplicate-leaf
handling, and virtual-loss unwind. Evaluation runs under `model.eval()` and
inference mode, and rejects non-finite values or misaligned priors. GPU
reproducibility is a separately declared bounded policy, not assumed from a
seed.

Tensorization starts in Python and is measured on representative development
roots at batch sizes 1, 8, 32, 64, and the intended maximum. Report p50/p95 for
projection, FFI, tensorization, collation, model forward, transitions, and whole
move latency. Move encoding into Rust only if encoding plus FFI remains over 20%
of end-to-end move time. Before such a port, add an encoder-contract fingerprint
covering observation/action schema versions, scalar/category layout and order,
normalization, vocabulary order, and encoder implementation version, then require
corpus-wide Python/Rust golden equivalence. Vocabulary identity alone is not
enough. `sts_core` acquires no tensor or training dependency either way.

### D6. Root extraction is one authoritative streaming replay per trace

Fix duplicate metadata serialization, then add a verifier-owned capture mode
that replays each trace once and emits the authoritative simulated state whenever
it reaches the first actionable public combat decision. Observed room phase may
validate provenance, but never supplies or repairs a root. Stop the trace at its
first unsupported transition or diff, type the exclusion, and commit its emitted
roots only after strict verification of the trace succeeds.

Root IDs are versioned SHA-256 digests of canonical root bytes. Equal IDs with
unequal bytes are fatal rather than silently merged. Every root round-trips,
validates as the first player decision, and hashes to its filename. The manifest
records the authoritative corpus digest, per-trace hashes, simulator/repository
revision, extraction schema/config, relative source identity, root hashes, and
all exclusions. A canonical membership digest excludes timestamps and absolute
paths; two regenerations must produce identical root bytes, membership, split
assignment, and digest.

Splits are provenance-component-stable. Before assignment, build the bipartite
connected components of source lineages and root SHA-256 values, so identical
root bytes and all lineages that produced them can never cross a split. Explicit
challenge provenance makes the initial component `real_trace_audit`; every other
new component uses SHA-256 over
`combat-agent-phase2-v1\0{canonical_component_id}` and a corpus-rank-independent
bucket: 70% `train`, 15% `development`, and 15% `sealed_test`.

Persist the resulting lineage/root assignment lock. On later regeneration, a
new lineage connected to one existing component inherits its split; a change
that would connect two locked components from different splits, or introduce
challenge provenance into a locked non-audit component, fails closed for review.
An unrelated component cannot move existing assignments. Each deduplicated root
retains every provenance entry. Loaders require the manifest and assignment lock
and reject a record whose root or split group is not in the requested split.
Default tools refuse sealed/audit splits; opening either requires an explicit
audited command and produces a report hash.

Roots are derived artifacts of the 433-trace lock plus a simulator revision and
are regenerated rather than edited. Captured traces remain immutable.

### D7. The hot path returns detached native-built public values, and CPU workers pin threads

Replace JSON serialization/parsing on the evaluator hot path, but preserve the
same pure fair boundary. Native code may construct detached
`FairCombatObservation` and detached `ActionDescriptor` values; tensor/model
code still cannot access `_native`, `RunEnv`, `full_state`, `snapshot`, private
action handles, RNG fields, or authoritative IDs. Keep the JSON surfaces for
debugging and persistence.

The native and JSON projections must be semantically identical over development
roots and under hidden draw-order, RNG, and internal-ID permutations. Projection
must consume no RNG and mutate no state. Extend the existing field-coverage and
non-interference tests to the native path.

Any CPU rollout/evaluation worker sets both intra-op and inter-op thread policy
before model initialization and parallelizes across processes. Batched GPU
evaluation is the other supported configuration. Stage 5 requires at least a
2x median decision-path speedup from the matched 122 us baseline, zero JSON
serialization/parsing in the measured evaluator path, and recorded p50/p95
end-to-end rollout rates with hardware and process count.

### D8. Training and evaluation are manifest-bound and resumable

Follow [`combat_rl_architecture.md`](combat_rl_architecture.md): behavior-clone
the policy from teacher actions and fit the value head only to completed,
unmasked outcomes, then initialize PUCT from that network and replace one-hot
teacher targets with visit distributions, reducing beam replay weight under a
versioned schedule.

Add a teacher-labeling job, manifest-validating dataset/replay buffer, versioned
training entry point, and paired evaluation harness. Symbolic records gain a
record version, root-manifest digest, and optional/masked value target for
truncation. Loaders resolve `root_id` and `split_group_id` through the named
manifest instead of trusting nonempty strings. Data-derived vocabularies are fit
from `train` only; development, sealed, and audit data only transform through
the frozen checkpoint vocabulary.

Current vertical-slice note: generated records and shards are digest-bound to a
validated root manifest, and training fits vocabularies from `train` only. Each
dataset now embeds canonical root-manifest bytes at a fixed relative path; the
loader validates both file and manifest digests and re-resolves every root,
split group, split, and canonical lineage membership without embedding root
snapshots. Root manifest V4 stores the requested seed cohort and a cohort
contract digest over the ordered canonical seed list, generator/source identity,
split salt, ascension, and `max_run_steps`; access mode and realized roots or
exclusions are excluded so ordinary and audited generation of the same seed list
share that digest. Dataset manifest V5 and training checkpoint format 3 also
bind that cohort digest plus a teacher/search contract digest covering teacher
name, version, and the full beam search config. Resume validates both digests.
Audited evaluation against a different realized root manifest requires
`allow_audited_split`, a valid audited-access dataset, and a matching cohort
digest; disjoint cohorts and teacher/search mismatches are rejected. Report V3
records official accuracy over every row, exact and truncated/nontruncated
numerators and denominators, truncated root count, `value_mae_rows`, and
per-record root, status, truncation, value-mask, correctness or error, and
value fields. These format bumps have no compatibility shim.

Training config V1 enforces the pre-frozen floor of 225 roots and 100 distinct
canonical lineages by default. Explicit lower thresholds are limited to tests
and smoke configurations. Checkpoint resume and evaluation also strictly bind
the source digest and runtime identity: Python, NumPy, Torch, platform basics,
deterministic CPU/thread policy, and the `pyproject.toml`/`uv.lock` digests.

Before training, freeze an experiment protocol containing root-manifest and
corpus digests, allowed split, model, vocabulary/encoder contract, objective
name and weights, optimizer, scheduler, batch size, steps, replay capacity and
beam/PUCT mixture, all Rust/Python/Torch seeds, deterministic action selection,
thread/device/software policy, transition and wall-clock budgets, truncation
specification, and checkpoint-selection rule. Resumable checkpoints are written only at batch
boundaries and include optimizer/scheduler/global step, RNG states, and a
content-addressed replay snapshot: immutable shard hashes, canonical example
IDs and order, producer root/episode/search position, active teacher/checkpoint
identity, eviction state, and sampler RNG/cursor. Resume verifies every shard and
continues from that exact producer and sampler state; a buffer name or aggregate
digest alone is insufficient. Crashes, illegal actions, timeouts, and
nonterminals stay in the evaluation denominator.

Generate intentionally hidden-equivalent paired roots and report privileged
teacher disagreement before behavior cloning. Validation selects candidates;
it cannot establish promotion while the sealed split remains closed.

### D9. Explicitly deferred

Belief and particle search. Public-history representation beyond the current
extension point. Alternative power/counter tokenization. Run-level value
handoff. Any change to the handcrafted terminal proxy weights beyond making them
versioned experiment config. Ascension above A0.

## Staged Work and Gates

Each stage ends on a measurable gate. Do not start a stage with the previous
gate red. Every implementation stage also runs formatting, Clippy, workspace
tests with the documented serial exception, and the authoritative 433-trace
gate.

**Stage 0 — Freeze contracts and baseline.** Record the 433-trace corpus digest,
simulator revision, historical July planner artifacts, `combat_episode_v1`
bounds, split salt/buckets, incumbent proxy version, and supported Python API
surface. The incumbent search objective remains unchanged for extraction and
paired comparisons; experimental proxy weights are versioned, and results with
different proxy versions are not compared. Gate: a checked-in experiment
protocol contains every reproducibility field required by D8, and the current
corpus is 433/433.

**Stage 1 — Fair episode boundary.** D1 and D2. Gate: zero fair-boundary failures
across 1,000 committed deterministic seed/policy episodes; explicit won, lost,
escaped, restored-terminal, and Proceed tests; both Secret cards projected with
hidden-permutation invariants; V1/V2 compatibility and cross-language action
schema tests green.

**Stage 2 — Episode contract.** D3. Gate: targeted tests reconcile all fields and
deltas for won, lost, escaped, and both truncation bounds; terminal detection
happens before another model decision; truncation is never a loss and contributes
zero value-loss weight.

**Stage 3 — Root corpus.** D6. Gate: 100% candidate accounting over the locked
corpus, no committed root from a failing prefix or trace, at least the historical
225-root floor and 100 distinct lineages, typed exclusions, SHA-256/round-trip
integrity, stable split assignments, identical canonical digest across two
regenerations, and extraction wall time no greater than twice one ordinary
corpus replay on the same machine. If the fixed floor is missed, stop and revise
the data plan before training rather than lowering it after seeing results.

**Stage 4 — Single teacher.** D4. Gate: `sts_search` extraction matches every
deterministic golden field on every development root, while serialized field
names remain compatible and timing is reported separately. The repository-unused
Python objective family was deleted in this extraction under the approved
no-shim policy. Generate hidden-equivalent root pairs and record teacher
disagreement before behavior cloning.

**Stage 5 — Throughput.** D7. Gate: at least 2x median native decision-path
speedup against the matched 122 us baseline, zero JSON operations in the
evaluator hot path, native/JSON semantic equality and non-interference tests,
correct CPU thread policy, and p50/p95 component plus complete-rollout rates on
representative development roots.

**Stage 6 — PUCT and batched evaluation.** D5. Gate: exact CPU tree equality
across three repetitions for the same checkpoint, batch size, backend, and
transition budget; no budget overshoot; public-choice edge bijection; evaluator
error/virtual-loss recovery tests; batch-size benchmarks; and a recorded
Python-versus-Rust tensorization decision under the 20% rule.

**Stage 7 — Expert Iteration candidate selection.** D8. Gate: on development
roots at equal transition budget, deterministic network-guided search has more
wins, turns no incumbent win into a non-win, introduces no illegal/error/timeout
outcome, improves paired HP fraction, and then compares potion value and action
count. Report wall-clock cost, paired confidence intervals, every per-root
regression, and the pre-frozen number of candidate-selection attempts. This
selects a candidate; it is not promotion.

**Stage 8 — Audited sealed evaluation.** Open `sealed_test` through the explicit
audited command and record its report hash. The tool records audited intent but
cannot enforce one-shot access against the local filesystem owner. Apply the same paired rule without
retuning. Then run `real_trace_audit` as an independent diagnostic gate. A
candidate that fails is rejected; the sealed split is not reused for tuning.
Only a passing Stage 8 agent may be described as beating or replacing the beam
incumbent.

`real_trace_audit` is reachable only through explicit challenge provenance.
Simulator-seed roots never satisfy that split. Stage 8 must report the audit
root count alongside its verdict; an empty audit split is an unmet gate rather
than a pass.

## Settled Phase-2 Policies

- Episode truncation is versioned as 512 accepted public decisions or 100 player
  turns, whichever occurs first; truncated value targets are masked.
- The incumbent terminal proxy is frozen for extraction and incumbent
  comparison. Alternative weights are versioned experiment configurations and
  cannot share comparison tables or checkpoint identity with another proxy.
- The repository-unused `py_sts` objective family was not a teacher and was
  deleted in this extraction under the approved no-shim policy; no legacy
  compatibility adapter remains.
- Training does not start below 225 roots and 100 distinct lineages. This floor
  is fixed from the historical benchmark before regeneration, not selected from
  the resulting learning curves.
- Stage 7 uses development only. `sealed_test` is opened under the explicit
  Stage 8 audit protocol; `real_trace_audit` remains a separately reported final
  diagnostic. Filesystem controls are operational, not cryptographic.
- Matched gameplay (`evaluate_matched_gameplay` / `sts-combat-rollout`) is a
  separate diagnostic from static imitation (`sts-combat-evaluate`). It restores
  each split-root snapshot independently, compares seeded random, greedy-network,
  and live beam episodes under the checkpoint teacher/search contract, keeps
  errors and truncations in the win-rate denominator, and does not promote a
  candidate.
