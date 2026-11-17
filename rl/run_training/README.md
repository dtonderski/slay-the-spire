# A0 run training

The sole training entry point is **`rl/train.py`**. `--task combat` (default) keeps
the existing combat loop. `--task run` dispatches to a separate implementation:

- `collector.py`: natural A0 runs, public macro decisions and frozen **sampled**
  combat (including escape potions); forced actions execute without actor loss.
- `model.py`: small complete-candidate macro scorer with a **new run-value head**.
  It uses deterministic hashed features of typed public observations, public
  current-act map in the ordinary context, previous noncombat action and revision-free action
  descriptors plus their resolved visible cards/offers/map destinations. The
  policy starts uniform and the fresh critic starts at zero. This is a correctness
  baseline: hashing has collisions and is
  **not** the planned lossless numeric run encoder / set-graph architecture.
- `token_model.py`: opt-in `--encoder tokens` macro model. One position-free
  token per deck card, relic, potion, reachable map node and legal candidate
  through a small transformer. See [Token encoder](#token-encoder).
- `trainer.py`: completed-episode Monte Carlo actor-critic (`gamma=1`), on-policy
  public-input recomputation checks, fixed held-out evaluation, logs/checkpoints.
- `parallel.py`: opt-in sharded collection in spawned worker processes; see
  [Sharded collection](#sharded-collection-across-processes).
- `rewards.py`: declared terminal reward components and outcome-based success.
- `metrics.py`: accepted behavior counts, healing by HP band, elite entries and shop re-entry diagnostics.
- `progress.py`: monotone public-history act, floor, key and Heart milestones.
- `diagnostics.py`: opt-in public choice/context audits and descriptive policy statistics.
- `contracts.py`: explicit schema-9 run outcomes and screen-based ownership.
- `restrictions.py`: declared policy eligibility for navigation commitments,
  optional visible Prismatic avoidance, direct smith and final card-reward
  closes; native legal choices remain unchanged.
- `targets.py`: tested reference GAE for later nonterminal fragment bootstrapping;
  **not used** by the initial completed-episode trainer.

## Bounded pilot

From the repository root, provide a trusted existing combat checkpoint and a
JSON list of held-out **decimal seed strings**, e.g. `["100001", "100002"]`:

```bash
uv sync --project rl --frozen
uv run --project rl --no-sync python rl/train.py --task run \
  --run-id a0-run-pilot --combat-checkpoint /path/to/combat/latest.pt \
  --validation-seeds /path/to/run-validation-seeds.json \
  --objective act1 --batch-size 2 --updates 1 --max-actions 5000 \
  --collect-only --wandb-mode disabled
```

Outputs default to `wandb/<run-id>/` relative to the working directory; choose a
new run ID every time, or set `--output-root`. `--collect-only` performs no updates.
Remove it for the experimental optimizer. `--objective act1` now trains on
`min(furthest_observed_act1_floor / 16, 1) + 5 * act1_cleared`: death on floor 8
returns 0.5, death at the boss returns 1, and accepted entry into Act 2 returns 6
(full progress plus clear bonus). This is computed once at episode completion,
not per action; errors/cutoffs receive no terminal reward. The macro value head
remains an unconstrained linear output trained against targets in [0, 6].
`act3` and `heart` retain binary rewards from explicit settled outcomes. Default starts
are **natural HP, A0**, with Heart profile enabled by default (`--no-final-act`
disables it for non-Heart objectives). No seeds, revisions, private state or
simulator futures enter the macro model.

## Full-Heart reward and observability

`--objective heart_progress` opts into the shaped full-run objective:

```text
3 * min(furthest_observed_floor / 55, 1)
+ 2 * first_three_acts_cleared
+ 30 * explicit_heart_victory
```

This is a **single terminal return**, not repeated payments for UI actions. Act
bonuses require observed entry into a later act or an explicit final victory;
floor numbers alone never establish a clear. The scale of 55 corresponds to the
native Heart-combat floor; TrueVictory on 56 is clipped (source:
`run/reward.rs::corrupt_heart_victory_proceeds_to_true_victory_floor`). This is
source-backed reward accounting, not new real-game parity evidence. Completed
non-Heart runs score at most 9; a Heart victory scores 39. Errors, cutoffs and
invalid boundaries still receive **no target**, even when earlier milestones
were observed. The old `heart` objective remains binary and unchanged.

Shaping can favor reliable progress over risky victory attempts; it is not an
exact proxy for win probability. The trainer archives `best-validation.pt` using
success rate first (Heart wins for Heart objectives), unknown counts second,
shaped reward third. Partial scheduled evaluations cannot select a best model.
Selection is over this invocation's repeated validation, not an independent test;
keep fresh final-test seeds separate. Native/source/profile provenance remains
archived, including any declared Prismatic restriction.

Use explicit `--warm-start-policy-only /path/to/best.pt` when changing objectives:
actor/shared-encoder weights transfer, critic heads retain their fresh
initialization with zero outputs, optimizer/RNG/counters start fresh. All model
shapes, feature/observation and frozen-combat contracts still must match. Ordinary
`--warm-start` still rejects reward/objective changes; exact resume remains strict.
This option does not restore, repair or transplant simulator states.

### Opt-in key exploration and bounded-action objective

`--objective heart_keys_budget --final-act` keeps the same floor/act/Heart
components, adds **+0.5 once per first observed key possession**, **+3 once for
observed Act-4 entry**, and subtracts **20** if an otherwise valid ongoing run
exhausts its declared accepted-action budget. Natural starts own no keys;
revisiting screens or selecting a burning elite route never grants another key
bonus. Entry credit requires accepted public history actually reaching act 4:
keys, a high floor or an Act-3 clear alone do not imply entry. It does not require
surviving the Act-4 elite; encountering the Heart grants no additional entry
bonus. The bonuses accumulate in the single MC task return, not as repeated UI
payments. Completed non-Heart credit is at most 13.5, Heart victory with all keys
scores 43.5, and an action-limit target is at most -6.5. The version-2 reward
protocol distinguishes this task from the previous key/budget protocol (which had
no entry bonus); old experiments retain their original pinned sources and labels.
This is an explicit bounded-budget training objective, not a claim that a
truncated game was lost. The legacy objectives above are unchanged.

The collector journals termination reason, actual public game outcome and budget.
A budget target requires the accepted-action count to exactly equal that budget
with an ongoing game. A real terminal outcome at the boundary takes precedence.
Signals, wall-clock stops, invalid boundaries, rejected actions and simulator or
infrastructure errors never receive this penalty or a partial key target. A stop
coinciding with budget exhaustion is conservatively recorded as interruption.
No simulator state is changed, repaired, retried or projected into a made-up death.

Batches of terminal games and genuine budget targets may learn together. A batch
with any missing target is still quarantined in full. Metrics deliberately
separate **game completion** from **training-target availability**: action limits
stay in unknown game outcomes and cutoff counts, not deaths/completed games, even
when they have a valid task target. Reward means and critic MSE include labelled
budget returns; `reward/target_samples` exposes their denominator separately from
`reward/terminal_samples`. `reward/act4_entry_bonus_mean` exposes the new component;
`progress/act_4_entries_observed` counts entries even for unlabelled episodes.
Checkpoint selection still prioritizes Heart wins.

`keys.py` measures Ruby recall, Sapphire selection, Emerald pickup, and the
separate choice of a publicly marked burning-elite destination. It records native
and policy-eligible opportunities, accepted choices, mean policy probability,
forced-choice counts and optional-choice rates. No opportunity means an omitted
rate, not zero preference. Unique `(act, floor)` opportunity/choice counts provide
a second view less sensitive to menu revisits (not a claim of distinct physical
rooms). Already-held keys are excluded, proposals count only after acceptance,
and taking an Emerald route is never reported as acquiring its key. Existing
observed-possession milestones remain separate. Telemetry draws no RNG and does
not enter policy features.

Reward changes require a new run identity and explicit policy-only transfer,
with critic, optimizer and sampler reset. This objective can incentivize early
death to avoid a timeout or key grabs at the expense of survival; track deaths,
key-choice rates, Act-4 access and Heart wins, not just shaped return.

### Opt-in campfire/key exploration trials

`--macro-exploration none|fullhp|rest_keys_t4|rest_keys_t8` declares an
experimental policy-distribution change. `none` preserves the original scores.
All other profiles exclude `rest_heal` at exactly full public HP only when an
already eligible smith/recall alternative exists and Dream Catcher is absent.
This is a restriction, not a universal claim that rest has no possible side effect;
native actions, candidate indices and simulator legality stay unchanged.

The `t4`/`t8` profiles additionally divide macro logits by 4/8 at rest screens and
visible optional Sapphire-choice screens. Other decisions remain untempered.
Temperature does not favor a particular candidate, force/grant keys or peek at
future offers. It can increase harmful low-HP nonhealing and room-exit choices:
monitor survival, skipped campfires, actual key acquisitions and Act-4 access.

Temperature is stored in each public-derived macro input and applied in model
forward for collection, audits, pre-backward recomputation, actor gradients and
entropy alike. Existing finite/mask checks and recomputation tolerances remain
fatal. No sampling-only/off-policy exploration is used. All native candidate rows
remain represented. Profile/protocol are archived in config; journals identify
profile and per-decision temperature. Exact resume rejects profile changes;
weights-only transfer records the parent profile and resets optimizer/RNG/counters.
No parameter shapes or reward accounting change. These experiments do not fix or
prove the cause of weak HP conditioning; temperature and masking are separate
from the explicit-health architecture ablation below.

### Metrics: observation versus explanation

- **Outcomes/coverage:** Heart wins, non-Heart Act-3 completions, deaths by act,
  observed act clears, key collection, Act-4 entry and Heart encounter. Milestones
  remain visible for incomplete episodes without treating those episodes as wins
  or losses. Counts carry an observed-progress coverage denominator. Validation
  also reports success and Heart-win lower bounds over **all scheduled** cases,
  including unattempted cases; completed-only rates are explicitly conditional.
- **Efficiency:** collection/update time, episodes and actions per second,
  optimizer updates per wall-clock hour, episodes actually used for learning,
  and completed episodes discarded with incomplete batches. Whole-batch
  quarantine is unchanged for episodes lacking a task target. Genuine budget
  targets are learnable only under the opt-in bounded objective above. Same-floor
  shop re-entries are measured, not forbidden.
- **Behavior:** accepted card-taking/closing, purchases, potion use, rest choices,
  and elite entries. A closed card menu is **not** necessarily a skipped reward;
  repeated menus are decision-level observations, not independent opportunities.
- **Policy health:** entropy, sampled-choice probability, concentration, and
  reward-target value MSE, plus pre-clip gradient norm and clipping indicators
  on actual optimizer updates. Policy/value statistics are decision-weighted on retained
  accepted macro records (errored episodes currently drop their macro records).
  Critic values predict shaped return, **not calibrated Heart-win probability**.

`--decision-diagnostics evaluation` records all ordered candidates, eligibility,
probabilities, chosen index/value and public deck/relic/HP/key context inside
attempt journals during validation. `all` also records training; default `none`
avoids the extra serialization/storage. Use bounded audits: menu loops can make
these records large. Rejected attempts remain proposals, not accepted choices;
the existing acceptance fence is authoritative. Masked logits serialize as null
with probability zero, never non-standard JSON infinities. No RNG is drawn for
telemetry, and no diagnostic/milestone counters enter policy inputs or rewards
apart from the explicitly declared public-progress reward components.

This provides **observability** and evidence for subsequent interpretability
studies, not proof of strategy or causal explanations. Conditional pick rates
must account for offered alternatives, affordability, constraints, floor/HP and
repeated-menu visits. Context ablations and paired continuation experiments are
separate analysis; no synergy score or claimed explanation is fabricated here.

## Environment limitations

Prismatic Shard still exposes unsupported cross-color gameplay. Strict Courier
purchase successors may require process-global libGDX draws absent from run-seeded
inputs. Headbutt run-boundary settlement has a separate simulator fix. These
simulator-only findings are not dedicated real-game interaction parity.

### Optional restricted Prismatic policy

`--avoid-prismatic` explicitly labels a **restricted-policy experiment**, applied
in both training and validation. Visible optional Prismatic Shard purchases and
relic-taking choices receive exactly zero policy probability. Their original
candidate rows and action indices remain intact; simulator legality, relic pools,
reward generation and game rules are unchanged. Only public offered identities
are consulted, never hidden outcomes. Prismatic avoidance is off by default;
navigation commitments below are a separate declared restriction.

Random/unknown grants and mandatory single-action grants remain possible. Their
unsupported successors still fail normally and quarantine batches; this option
is not a Prismatic implementation or a fix for shop-navigation cycles. If a
multi-choice decision has no eligible action, collection fails explicitly rather
than restoring excluded choices or retrying a game action.

Config/source archives and evaluation metrics identify the restriction. Journals
record its protocol at setup and the complete eligibility vector at macro
attempts. Collection and learner recomputation use the same masked distribution;
raw scores must still be finite for **every** actual candidate, including excluded
ones. Stored effective logits and chosen indices are checked before backward.
No observation/features, model shapes or frozen-combat ABI change. A restriction
change rejects exact resume; use a new run identity and explicit macro
weights-only `--warm-start` with fresh optimizer/RNG. Restricted repeated-validation
scores must not be presented as unrestricted or independent test performance.

### Explicit Courier environmental-input profile

`--training-environment-seed UINT64` opts natural runs into the existing native
private simulation-only environmental-input provider. Omission remains strict:
no automatic opt-in, repair, action retry, or failed-candidate filtering.
`environment.py` derives each episode input from a versioned SHA256 of the declared
master seed and canonical decimal run seed, independently of policy RNGs and
collection scheduling. Fixed validation seeds retain the same input on every
assessment. Neither seed becomes a policy feature.

The master seed/protocol is archived in config and checked on exact resume; each
allocated `training_rng_seed` is journaled at setup and used verbatim for
reconstruction. Existing journals without it retain strict semantics. Synthetic
root curricula inherit the original journal input and reject a new master-seed
option rather than substituting a different stream. This profile is **not** exact
vanilla global-RNG sequence replay or permission to restart a stopped experiment.
No Prismatic cards, checkpoint adapters, numeric schemas or vocabularies change
with this option.

The default **aborts** on simulator errors or incomplete training
batches. Both serial and batched modes log localized collection failures in
`collection-errors.jsonl`, including failures that abort the run. For an
explicitly authorized bounded experiment, `--continue-on-collection-failure`
quarantines the **whole batch**, logs the failed seed/action, and samples fresh
runs. It never retries a failed action, silently filters legal candidates, assigns
error/cutoff death rewards, or trains the successful remainder of a partial batch.
Model/encoding/numerical/infrastructure errors still stop immediately. This
experimental training distribution is **conditional on complete supported
batches**, potentially biased toward paths without simulator failures; it is not
an unrestricted full-game training claim. Evaluation includes error/cutoff counts,
all attempted seeds in its success lower bound, and any unattempted scheduled cases.
`--max-consecutive-skips` (default 8) stops a repeatedly failing campaign;
explicit `0` disables that campaign limit without changing whole-batch quarantine.

Use `--max-hours 8 --compress-journals` for a bounded overnight experiment.
For an explicitly operator-authorized indefinite campaign, use `--updates 0`,
omit `--max-hours`, and set `--max-consecutive-skips 0` with continuation mode.
Zero updates means unbounded batch attempts, not zero learning. SIGTERM/SIGINT
still stop cooperatively and commit the last valid boundary. Per-episode action
limits, complete-batch learning, public-input checks and fatal model/numerical
errors remain enforced; disabling campaign limits does not repair gameplay.
Monitor disk usage: immutable journals accumulate without a time bound.
Deadlines and SIGTERM/SIGINT request a cooperative stop between simulator calls;
a partial batch is discarded and the checkpoint records consumed sampler state.
Do not treat a signal-interrupted fragment as an exactly resumed trajectory.
This version deliberately does not bootstrap arbitrary combat/UI cutoffs using
an untrained macro critic. Investigate cycles rather than hiding cutoff coverage.

`config.json` records native, source, frozen checkpoint and validation hashes.
`metrics.jsonl` is available even with W&B disabled. `latest.pt` is atomically
written at processed batch boundaries and contains macro weights, optimizer,
RNG state, successful optimizer-update count and skipped-batch count. Periodic
validation also archives numbered checkpoints; `finished.json` records clean
completion and `failure.json` records a fatal stop. Source files are copied to
`source/` for reproducibility. With these artifacts, `--resume-from` restores the same protocol in a **new output directory**.
`--updates` bounds additional batch attempts; `optimizer_updates` counts actual
learning steps. The wall-clock limit can be changed on resume. Changes to simulator, source, features,
objective, seeds, controller, device or loss protocol reject exact continuation.
The combat checkpoint is a weight-only FP32 integration, not continuation of its
original training job. It is never optimized by this trainer. Public card vocabulary
v2 preserves all 322 v1 indices and appends Cold Snap and Piercing Wail. An old
322-row checkpoint requires explicit `--adapt-legacy-combat-checkpoint`: only those
two new embedding rows are initialized to zero; every old parameter/index is
preserved and all other shapes and parameter keys must match. This does not claim
bitwise logit equivalence, optimizer compatibility or competence with new cards.
The original checkpoint bytes/hash remain unchanged; config records the adapter
and source archives include the immutable legacy identity list.

After an intentional simulator/source update, use `--warm-start /path/to/latest.pt`
for explicit **macro weights-only transfer** into a new run. Architecture, feature,
observation, reward, objective and frozen-combat contracts must match (the explicit
policy-only option above is the exception for reward/objective changes); native/source
hash changes are allowed and parent checkpoint/native hashes are recorded. The
optimizer, sampler and local iteration counters start fresh. This is not exact
resume and does not restore simulator states or unfinished episodes.

The current-map input uses hashed feature version **4** and health version **5**.
Earlier map-history checkpoints are not exact resumes or automatically compatible
warm starts. For intentional **weights-only** transfer, use
`--warm-start OLD.pt --adapt-legacy-map-checkpoint` (or the explicit
`--warm-start-policy-only` option when also changing objectives). This narrowly accepts schema-8
hashed v2 (including missing legacy encoder), explicitly tagged hashed v3, or
health v3 into the corresponding schema-9 encoder. It preserves parameter values,
not predictions, optimizer moments, sampler state or the former policy. All other
architecture/reward/objective/frozen-combat checks remain enforced. The parent
schema/feature adapter is recorded; unsupported combinations fail closed. Use a
new run ID and a fresh training seed for this weights-only transfer so resetting
the sampler does not repeat the old run's initial training sequence.

Checkpoints commit before telemetry publishing, so a W&B connection failure cannot
lose an already processed optimizer update. For a manual graceful stop, send
SIGTERM to the Python PID in `trainer.pid`, rather than terminating the entire
service process group (which can kill W&B's helper before it flushes).

Validation sampling uses separate fixed macro/combat RNGs and never consumes the
training seed sampler. Every held-out seed is excluded from training draws.
Collection and learning are batched by default as described below. Stronger
encoders, recurrent history and PPO remain separate research questions.

Future work and investigation ideas are tracked in the repository-wide
[`TODO.md`](../../TODO.md).

## Navigation commitments

Default collection uses `commit_room_and_merchant_exit_v1`. The current public
map is already in every observation, including at Neow, so the actor need not
navigate UI screens to inspect routes.

- Leaving a room commits to advancing: `return_to_room` remains a native legal
  candidate but has zero policy probability.
- An accepted `leave_shop` means **finish shopping** in that room. The policy
  cannot re-enter that merchant. First entry, purchases, removals and choices
  inside the merchant remain available before leaving. Actual node entry resets
  this commitment; a later shop is independent.

These are deliberate strategic commitments, not simulator fixes or a claim that
all removed UI actions were meaningless. They can forfeit unclaimed rewards or
remaining purchases when the policy leaves prematurely. Raw candidate indices
are retained. Eligibility is journaled, stored with learner inputs, and applied
identically to sampling, recomputation, actor loss and entropy; excluded rows get
zero probability/gradient. Cutoffs and simulator failures keep their existing
quarantine semantics. Other possible loops (e.g. grid cancel/reopen or combat)
are not claimed eliminated.

`--raw-navigation` is an explicit ablation restoring native navigation choices;
it is not an exact resume of the old observation protocol. `--avoid-prismatic`
retains the live experiment's separate public-only restriction on optional,
visible Prismatic acquisitions; it does not reject unknown or mandatory grants.
Both protocols are archived and checked on exact resume. Neither modifies native
legality, captured traces, reward targets or action application.

## Batched execution and throughput

`--task run` now defaults to **`--collection-width 32 --learner-batch-size 128`**.
The first bounds concurrently active episodes; the second bounds macro decisions
per learner forward/backward chunk. Neither changes `--batch-size` (episodes per
optimizer update). Width cannot exceed the currently scheduled episode batch;
do not increase the learning batch size merely to fill the GPU.

- A single cooperative scheduler owns independent journaled trajectories. It
  batches combat and macro inference, not game transitions. Each episode retains
  its own policy RNGs, history, budget, revision checks and immutable journal.
  Results and seed/HP draws remain in input order, not completion order.
- Ragged weighted embedding bags and padded candidate scoring retain every legal
  choice, including escape potions. Padding is never sampled. Each run samples
  its true-length row with its own generator, without global policy RNG draws.
- Public hash paths, immutable public map contributions, candidate facts, complete
  public contexts (including map/action history), and host feature columns have
  bounded caches. Cached entries contain no learned activations, device tensors or
  hidden state; they remain valid across optimizer updates. Feature buckets use
  traversal-order FP64 accumulation, retaining touched zero-weight buckets. Cached
  host columns are read-only NumPy arrays, concatenated before the two pinned
  uploads rather than repacked through Python scalar lists. Collision accumulation
  order and packed scalar order remain unchanged.
- Macro dense heads use canonical 128-row FP32 tiles in scalar collection,
  batched collection and learner recomputation (`macro_fixed_row_tiles128_fp32_v1`).
  Public feature pooling is shared by both forward entry points. Padding is
  internal to row-local Linear/Tanh/ReLU heads and contributes no loss/gradient.
  This prevents cohort-dependent dense-kernel rounding; tile size is not a
  collection/learner-width tuning knob. Parameters/features/schemas are unchanged.
- Collection forwards use `inference_mode`; only public inputs and plain behavior
  predictions survive into learning. The learner recomputes ordinary autograd
  tensors under current weights. Healing probabilities are exported only for
  batches with healing opportunities, without changing any policy RNG draws.
- The learner packs public inputs once, verifies all inference/recomputation
  pairs **before any backward**, then backpropagates chunks. The existing finite,
  eligibility, logit and value checks retain `atol=1e-6, rtol=1e-5`; actual
  mismatches remain fatal and now include bounded numerical details in the
  failure artifact. Actor sums are still
  divided by episodes; value/entropy terms are still divided by macro decisions.
  There is one gradient clip and one optimizer step per complete episode batch.
- A failed action is never retried. Explicit continuation mode still quarantines
  the whole learner batch. Unlocalized collective numeric-export failures and
  model/infrastructure errors are fatal, not per-episode death labels. Open peer
  journals receive cancellation records. Cooperative stops finish in-flight
  decisions, launch no new episodes, and leave incomplete batches untrained.

For the serial reference path use
`--collection-width 1 --learner-batch-size 0`. Gradient/loss reductions can still
vary with chunking; bitwise optimizer equivalence with historical serial learning
is not promised. Forward head arithmetic is now cohort-shape independent within
one backend; this is not cross-device/PyTorch-version bitwise equivalence.
Execution settings and source hashes are part of strict resume validation.
Changing execution mode requires a new identity; use compatible weights-only
warm start rather than bypassing resume checks. The fixed-tile forward protocol is
archived separately. Historical cohort-shaped FP32 weights transfer without a
parameter adapter, but logits can change slightly: preserve the parent checkpoint,
start fresh optimizer/RNG, and do not claim an exact continuation. Default combat-task training is
unchanged, and this does **not** change macro rewards or fix weak learning.

An October 2026 fixed-weight benchmark on one RTX 5080 included collection,
compressed journals, one real Adam update and checkpoint output. Warm medians:

| Workload | Serial | Batched | Gain |
| --- | ---: | ---: | ---: |
| 64 natural starts, width64 | 24.36 s | 6.21 s | 3.92× |
| 128 campfire starts, width32 | 5.31 s | 1.39 s | 3.82× |

Each uses two warm trials after one cold trial; all commands, accepted revisions
and outcomes matched across implementations (12,428 and 2,968 actions per trial).
Learner-only gains were about 16× and 21×, **not end-to-end speedups**. A complete
16-update, batch16 CLI reference took 47.44 s. Two optimized checks of the same
work took 26.42 s and 14.82 s (1.80–3.20×), including imports, cold root
reconstruction, validation and local telemetry (W&B disabled). Whole-job timings
showed more variation; do not extrapolate warm throughput to every job. These are
historical, workload-specific measurements of the earlier observation/navigation
protocol, not guarantees for the new map inputs and not learning-strength claims. All 448 CLI episode
setups and execution fingerprints matched. A CUDA split/resume check reproduced
the uninterrupted 16-update model, optimizer and saved RNG states exactly.

`train/collection_seconds`, `train/update_seconds` and
`validation/collection_seconds` expose phase timings. Combat steps use
`State.advance`, which returns an ongoing combat successor as a `CombatStep`
(run-level fields only) instead of a typed observation: the combat policy reads
`numeric_decisions` tables and the loop reads only act, floor, HP, keys and
monster keys, so journals and episodes are byte-identical while each combat step
skips the typed projection (~200 → ~80 µs per combat step in isolation). Typed
projection of non-combat screens and public feature assembly remain CPU costs.
Native `numeric_steps` is deliberately not used: its non-atomic batch failure
contract needs trustworthy per-state settlement accounting before replacing
individually journaled transitions, and it only uses threads for chunks of at
least 64 states.

### Sharded collection across processes

One trainer process collects on one core. `--collection-shards S` (default 1)
splits each job list into `S` contiguous shards, `jobs[n*k//S : n*(k+1)//S]`;
each shard is one cooperative `collect_many` call of `--collection-width` and
results are concatenated in job order. `--collection-workers N` (default 0)
only decides where shards run: `0` runs them one after another in the trainer,
`N > 0` in `N` persistent spawned processes (`parallel.py`), each a full
collector copy with its own macro model (same train mode as the trainer),
frozen combat model and inference device: `--collection-worker-device`
(`cpu` or `cuda`, default `--device`; requires `N > 0`). The learner stays on
`--device`. Weights travel with every shard; workers
ignore SIGINT/SIGTERM and stop through a flag only the trainer sets. Sharding
requires natural starts and `--collection-width > 1`; `S=1, N=0` is the
unchanged collector.

For fixed source, worker device, weights, jobs, width and `S`, every episode (choices,
stored logits/values, public inputs, statuses, rewards) and every journal is
bit-identical for every `N`, including `S=1, N=1` against the in-process path
on CUDA. Changing `S` changes which episodes share batched forwards, so
logits can round differently (accepted commands matched in the measurement
below, but that is not promised). CPU and CUDA inference round differently too,
so trajectories differ between worker devices. Exact resume therefore checks
`collection_shards` and `collection_worker_device` but ignores
`collection_workers`. Failures keep the abort
and whole-batch quarantine semantics; a failed shard stops its peers and its
exception is re-raised in shard order; a dead worker is fatal.

November 2026, RTX 5080 / Ryzen 9 7900X (12 cores), token macro model, natural
`act1` starts, width 64, warm collection seconds (range over warm trials), measured
while another training run used ~5 cores; the update adds ~0.5 s (64 runs) to
~2.5–3 s (256 runs). "Before" is the code without `State.advance`:

| Runs | S=N | Worker device | Before | After | PSS (all processes) |
| ---: | ---: | --- | ---: | ---: | ---: |
| 64 | in-process | CUDA | 13.7–14.6 s | 11.2–12.4 s | 2.2 GB |
| 64 | in-process | CPU | 11.5–12.4 s | 9.5–10.2 s | — |
| 64 | 4 | CUDA | 8.0–9.6 s | — | 7.0 GB |
| 64 | 8 | CPU | 3.6–3.9 s | 3.8–4.1 s | 4.9 GB |
| 128 | 8 | CPU | 4.8–8.4 s | 4.5–9.3 s | 5.7 GB |
| 256 | 4 | CUDA | 16.7–17.4 s | — | ~7 GB |
| 256 | 4 | CPU | 13.8–14.5 s | 11.6–12.1 s | 4.6 GB |
| 256 | 8 | CPU | 7.6–19.0 s | 6.7–11.3 s | 6.2 GB |

Journal and episode fingerprints were identical before and after wherever both
were measured.
Per-step work is linear in steps (~0.65 ms per accepted action per process on CPU
for 16–64 runs per shard), so throughput scales with processes, not batch size.
CUDA workers scale badly because concurrent processes time-slice one GPU: with
the same 16 runs in each process, per-process time rose 1.4× at 6 and 2× at 12
CUDA processes but stayed flat (≤1.2×) on CPU, and CPU inference is also faster
for these tiny models. A CPU worker uses ~0.45 GB instead of ~1.25 GB. Use
`S=N` (more shards than workers ran slower: 16 shards on 8 workers took 9.0 s
against 7.7 s) and at most one worker per free physical core; the longest run
(up to ~1,700 actions) bounds small batches. Recommended on this machine:
`--batch-size 256 --collection-width 64 --collection-shards 8
--collection-workers 8 --collection-worker-device cpu`. The first batch after
start-up also pays process spawn and warm-up.

`--collection-overlap-inference` (off by default) runs each wave's macro
forward on a helper thread while the collector thread scores combat. Rows,
models and per-episode generators are independent and the helper inherits the
single intra-op thread, so episodes and journals are unchanged (tested, and
fingerprint-identical on 256 heart runs); a collector then uses up to two busy
cores. Resume ignores it. Worker processes also freeze start-up objects out of
garbage collection and raise the generation-0 threshold to 10,000 (the default
spent ~4% of shard time in collection passes).

Where a shard's time goes (November 2026, one in-process shard = shard 0 of
`S=8`: 32 natural `heart_keys_budget` runs, width 64, CPU, trained token macro
checkpoint, 11,810 accepted actions, 3,293 macro decisions; wrapper timers,
exclusive time, machine shared with two training runs):

| Phase | Before ms/action | After ms/action |
| --- | ---: | ---: |
| macro forward (503 batched calls) | 0.17 | 0.18 |
| combat forward (638 calls) + numeric export | 0.15 | 0.17 |
| native step and `State.advance` | 0.10 | 0.06 |
| typed observation decode (3,750 non-combat successors) | 0.08 | 0.04 |
| macro token encoding | 0.03 | 0.03 |
| garbage collection | 0.02 | 0.003 |
| journal (JSON, gzip write and per-row flush) | 0.03 | 0.03 |
| sampling, bookkeeping, generator loop | 0.05 | 0.05 |

The forwards are compute-bound at one thread (a typical macro batch is ~7 rows of
~70 padded tokens) and cannot change shape without changing rounding, so they
now dominate. Before the change, most of the native step was converting the
successor observation to records: the pure transition is ~15 µs of ~60 µs.
Native `State.advance` now reads an ongoing combat successor's run-level fields
from the fair projection without record conversion, and returns other successors
as one observation mapping; unchanged map nodes reuse their previous decode.
Uneven shards leave 13–19% of worker time idle at the batch tail; work stealing
would change which runs share a forward and therefore rounding, so it is not
done. Result pickling and transfer to the trainer take ~0.2 s per 256-run batch.

256 runs, `S=N=8`, CPU workers, same settings, warm collection seconds (trials
3–8 of 9, run back to back while two 8-worker training runs shared the machine,
so expect ±20% noise; every trial fingerprint-identical):

| Code | Mean | Range |
| --- | ---: | ---: |
| before | 10.9 s | 10.3–11.3 s |
| after | 8.9 s | 7.4–9.8 s |
| after, `--collection-overlap-inference` | 8.1 s | 6.9–9.6 s |

The bounded comparison tool is [`benchmark_run_training.py`](../tools/benchmark_run_training.py).
It is not an alternative training entry point. It resets fixed macro weights per
trial, pins combat/native/source hashes, records complete-case coverage and
execution fingerprints, and skips the entire optimizer update on any unresolved
episode. The macro checkpoint's `encoder`, `direct_smith` and
`final_card_skip` settings are used for collection, in train mode like the
trainer; `--final-card-skip` turns the last one on for any checkpoint.
`--shards` and `--workers` (natural starts only) use the same sharded
collection, with `--worker-device` (default `--device`) as the worker inference
device; results then include exact per-episode and full-journal hashes. Example (use a new directory):

```bash
PYTHONPATH=rl uv run --project rl --no-sync python rl/tools/benchmark_run_training.py \
  --profile natural --macro-checkpoint /path/to/macro/latest.pt \
  --combat-checkpoint /path/to/combat/latest.pt --seeds /path/to/seeds.json \
  --episodes 64 --trials 3 --width 64 --learner-batch-size 128 --out /path/to/new-benchmark
```

Both benchmark profiles default to binary Act-1 completion targets; `--objective act1`
selects the trainer's shaped objective and `heart_keys_budget` the full-run
objective (runs ~1.8× longer than Act 1). `--collect-only` skips the update;
`--overlap-inference` matches the trainer flag. Match the experiment's `--max-actions`,
`--adapt-legacy-combat-checkpoint` and (natural starts only)
`--training-environment-seed` explicitly. For `--profile campfire`, also supply
`--root-manifest`; initial reconstruction is included in cold timing.

`--cprofile` writes `trial-N.pstats` for collection, learning and checkpoint output,
excluding imports/model loading. Inspect with `python -m pstats PATH`. These are
CPU-call profiles, not CUDA kernel timings, and include profiler overhead: use
separate **unprofiled** trials for speed comparisons. Manifests pin seed-file,
RL source, Python observation-decoder, native and checkpoint hashes, plus NumPy,
PyTorch and device information. Results include terminal per-episode policy RNG
state hashes as well as execution fingerprints. Run comparisons
sequentially on the same hardware, retain every journal, and report warm/cold
results separately. Report whole-batch quarantine separately from successful
optimizer updates; a faster discarded batch is not higher learning throughput.
A serial baseline can be run under its own checkout's `PYTHONPATH` with
`--width 1 --learner-batch-size 0`.

## Synthetic pre-boss campfire experiment

This separate curriculum starts from reconstructed natural action prefixes at
unused floor-15 campfires, then configures **only current HP** on independent
synthetic initial states. Inventory, maximum HP, settled entry effects and RNG
are unchanged. No simulator state is hydrated from observations. Root seeds and
provenance are logged, never policy features. The original journals are copied
unchanged, with hashes; they are simulator diagnostics, not real-game traces.
The builder, bank loader and prefix reconstructor require a natural-start setup:
legacy journals may omit `initial_state`, but an explicit protocol must be
`natural_start`. Synthetic or unknown protocols are rejected before state creation.

Prepare a bank from an existing natural-run training output (requires both
training journals and a completed scheduled validation):

```bash
PYTHONPATH=rl uv run --project rl --no-sync python rl/tools/build_campfire_roots.py \
  --source-run /path/to/natural-run-output --evaluation-iteration 440 \
  --train-count 128 --validation-count 32 --out /path/to/new-root-bank
```

The builder records rejected sources and fails if the requested counts cannot be
met. Its narrow eligibility condition (reached pre-boss campfire, legal heal and
smith) is a declared curriculum distribution, not an unrestricted training claim.
Splits are by original run seed, not HP variant. Banks pin the native hash and
validate every accepted prefix on construction; source changes require rebuilding
and validating a new bank, never rewriting original journals.

Use the same training entry point:

```bash
uv run --project rl --no-sync python rl/train.py --task run \
  --run-id campfire-hashed-pilot --combat-checkpoint /path/to/combat/latest.pt \
  --validation-seeds /path/to/new-root-bank/validation-seeds.json \
  --root-manifest /path/to/new-root-bank/manifest.json \
  --objective act1_binary --encoder hashed --root-hp-min 0.1 \
  --root-eval-hp 0.15 0.5 0.85 --batch-size 16 --updates 128 \
  --eval-every 64 --max-hours 0.5 --compress-journals --wandb-mode disabled
```

`--root-eval-hp` values must yield distinct rounded-percent metric names; e.g.
0.149 and 0.151 are rejected because both would write `root_hp_15`.

Repeat with a new run ID and `--encoder health` for a matched ablation. Both use
fresh weights, the same initialization seed, root/HP sampling and frozen combat;
neither installs a healing heuristic or removes legal candidates. `health` adds
zero-initialized-output ReLU residual paths using normalized public HP/max-HP for
value and candidate scoring, bypassing the pooled tanh context. It adds parameters
and explicit HP access, so it is a feature/architecture ablation, not proof about
saturation alone. `health` is retained only as an opt-in research ablation for
controlled critic/representation follow-ups, not as a recommended training recipe;
the negative pilot below did not establish a benefit. `hashed` remains the default,
ignores the extra health tuple and retains the original
mathematical encoder. Feature version changes make old checkpoints incompatible
with strict continuation. Existing natural-run checkpoints remain preserved;
exact resume requires restoring their original source and matching native build.

### Token encoder

`--encoder tokens` selects `token_model.py`. The hashed encoder keys every deck
card by its tuple index and sums the whole context, map included, into one
vector, so it cannot count cards or relate an offer to the deck. The token model
instead builds one sequence per decision:

```text
[summary] [deck card]... [reward card | shop offer]... [relic]... [potion slot]... [map node ahead]... [candidate]...
     │                                                                                                    │
   value                                                                                      one logit per candidate
```

- A card token (deck, reward card, shop offer or candidate) is its base identity
  plus upgraded/bottled flags, with upgrade level, cost, cost-modified,
  temporary and its persistent dynamic value (Rampage, Ritual Dagger, Windmill
  Strike, Steam Barrier) as numbers. Deck order is not an input
  (`test_run_training_token_model.py` checks this).
- A relic token carries its public counters as numbers, sorted by counter key;
  the relic identity fixes which keys exist (at most three, Orange Pellets in
  combat).
- The summary token carries HP, gold, floor, keys, act, boss and screen tags,
  including the reward screen's `card_reward_flow`.
- Map tokens are the nodes reachable from the current position with their
  distance; a map candidate also carries the room mix that lies beyond it.
- Candidates sit in the same sequence, so an offered card attends to held cards.
  Buy candidates and `open_shop_remove` carry price and affordability;
  `toggle_grid_card` carries whether that card is selected.
- Reward cards: whenever a reward screen shows `screen.cards` (an ordinary card
  reward is public before it is opened), each card is a context token with its
  own role tag. `open_card_reward` can therefore score the offer it would open,
  and while the card screen is open each `take_card_reward` candidate also sees
  its alternatives. Queued rewards (Prayer Wheel etc.) expose only a count.
- Shop offers: every unsold card, relic and potion of an open merchant is a
  context token with price and affordability, so items the player cannot afford
  (which are not candidates) are still visible.
- Identities use exact embedding rows; free text (action kinds, event choice
  labels) is hashed into a separate range. `feature_version` includes a hash of
  the public card/relic/potion catalog, so a catalog change refuses old weights.

Not encoded: card type and rarity (only the Rust catalog in
`sts_core/src/content/cards.rs` has them; neither the observation nor the
Python package does), Match and Keep cards (choice labels map to grid positions
through a reordering, so a position-free token cannot be tied to its option),
and map edges beyond distance. Event options are a hash of event and label text,
so numbers inside a label are not features. Weights are not interchangeable with
`hashed`/`health`; start fresh.

`tokens-v2` added everything above beyond identities, upgrades, map and
candidates. Its numeric input is wider than `tokens-v1`, so v1 checkpoints
neither pass the exact `feature_version` check nor load; v2 training starts from
scratch. On 2,953 random-policy macro decisions the mean sequence grew from 57.0
to 58.1 tokens (max 97 both; a shop adds at most 13 offer tokens), the per-token
id width stays 6, and encoding costs about 55 instead of 47 µs per decision.

Attention over differently padded batches is not bit-identical, so the
on-policy recomputation check uses `1e-4` tolerances for this encoder instead of
the hashed model's canonical-shape `1e-6`/`1e-5`.

`--direct-smith` is a separate declared restriction (`campfire_direct_smith_v1`):
when direct `rest_smith` actions are offered, `rest_open_smith` gets zero
probability. The upgrade screen offers the same upgrades, and the previous
policy learned to open and cancel it about 36 times per run. Game legality is
unchanged and old checkpoints continue with it off.

`--final-card-skip` is another declared restriction
(`card_reward_close_is_final_v1`): closing a card reward without taking a card
is final, and `open_card_reward`/`open_queued_card_reward` get zero probability
for that reward afterwards. Reopening was free, and in November 2026 training
journals (train iterations 100–149, 12,032 runs) 66% of macro decisions and 34%
of all accepted actions were open/close of the same card reward; after at least
one close a card was still taken 89% of the time. Taking, Singing Bowl, other
rewards and other card rewards on the same screen (Prayer Wheel, Tiny House,
Orrery, events) stay available. The bookkeeping (`ClosedCardRewards`) uses only
public reward screens and accepted actions: `open_card_reward` reopens the shown
`screen.cards` if any, else queued slot 0; queued rewards are tracked by public
slot, which shifts down when an earlier queued reward is used. The record lasts
while the run stays on rewards, the map overlay (`idle`/`map`) or a grid opened
from the rewards; any other screen starts afresh. A reward screen always offers
`skip_reward`, and opens are only masked when some other action exists. The
simulator also offers `skip_reward` on an open card screen, which consumes that
reward; it is not used here because real-game replay maps `SKIP` on an open
card screen to `close_card_reward`. Journals record the protocol in the setup
row only when enabled, so default journals are unchanged. Old checkpoints
resume with it off; weights transfer with `--warm-start`. On a checkpoint
trained with reopening available (benchmark, 64 natural `act1` runs), macro
decisions fell from 12,571 to 4,188 and the longest run from 1,563 to 86, but
cards taken fell from 455 to 225 and clears from 32 to 19: that policy used
reopening to resample its take-or-close choice, so turning the flag on changes
the policy and is not a free speedup.

Why this exists and what was measured:
[project history](../../docs/project_history.md#macro-encoder-diagnosis-november-2026).

`act1_binary` gives **1 only on accepted entry into Act 2, 0 on death**, with no
pre-earned floor reward. Errors/cutoffs still have no targets. HP fractions are
sampled uniformly in `[root_hp_min, 1]`, rounded to positive integer HP. Evaluation
uses each validation root at every fixed HP fraction, with matched policy RNG
seeds across variants. `validation/root_hp_XX/*` reports per-stratum scores and
initial-value mean/MSE. HP variants of a root are correlated; uncertainty analysis
must group by original seed. These are conditional synthetic-root results, not
natural-start clear rates. Do not mix them into the natural-run learning curve.

The initial bounded experiment did **not** improve held-out clearing; explicit
HP paths alone did not teach HP-conditioned healing. See the settled result and
limitations in [project history](../../docs/project_history.md#run-level-health-conditioning-and-campfire-pilot-october-2026).

## Training and W&B metrics

`--wandb-mode online` publishes metrics; `tracking.json` stores the run URL.
`metrics.jsonl` always records the same scalars locally. Training logs every batch
attempt (including discarded batches); validation logs at `--eval-every` intervals.
W&B uses the explicit `iteration` axis for both, including evaluations at the same
iteration as a training batch.

- `train/healing/hp_00_25/{heal_rate,heal_probability,opportunities,heals}` and the
  corresponding `hp_25_50`, `hp_50_75`, `hp_75_100` series. Bands use pre-choice
  HP/max HP: [0,25), [25,50), [50,75), [75,100] percent. Count accepted rest-screen
  decisions only when healing was legal. Menu re-entry may create multiple
  decisions per campfire; forced heals count with probability 1. No opportunity
  means a zero count and an omitted rate, not a fabricated zero healing rate.
- `train/map/elite_percent`: 100 times elite map-node entries divided by all
  map-node entries. `nodes_visited` and `elite_nodes_visited` give the counts.
  Count successful `choose_map_node` steps, not UI actions or downstream options;
  burning elites count as elites. This measures exposure, not opportunity-adjusted
  preference. Event-triggered fights do not create additional map-node entries.
- `train/mean_floor`, `mean_furthest_act1_floor`, and
  `train/reward/{mean,progress_mean,clear_bonus_mean,terminal_samples}`.
  Reward means include completed episodes only. Clear rates use explicit success
  outcomes, **not shaped reward magnitudes**: a death with reward 1 is not a win.
- Every metric also has a `validation/` counterpart. Behavior counts include
  accepted prefixes of failed/cutoff episodes even when a batch cannot train.

Protocol v3 changes the Act-1 reward and metrics. Old binary-reward checkpoints
are not exact continuations; strict resume rejects them. The combat policy,
complete legal-candidate coverage, macro feature encoder and loss normalization
are unchanged. No heuristic healing/reward-collection overrides are installed.

## Research utilities

Historical investigation: [`../docs/run_level_training.md`](../docs/run_level_training.md).
Its original measurements predate the merged API prerequisites and this trainer.
`probe.py` and `replay.py` remain separate non-learning research utilities:

```bash
PYTHONPATH=rl uv run --project rl --no-sync python -m unittest discover \
  -s rl/tests -p 'test_run_training_*.py' -q
uv run --project simulator/python python rl/run_training/probe.py \
  --mode natural --ascension 0 --style exercise --count 32 \
  --out tmp/my-run-interface-probe
PYTHONPATH=rl uv run --project simulator/python python -m run_training.replay \
  tmp/my-run-interface-probe/case-0.jsonl
```

Research probe journals and trainer journals have different formats; `replay.py`
currently handles **probe** journals only and refuses native hash mismatch. Both
are simulator diagnostics, **not real-game traces**. Unit tests, successful pilot
updates and synthetic fixtures do not establish real-game parity or playing
strength. The reviewed real-game corpus remains the gameplay regression evidence.
