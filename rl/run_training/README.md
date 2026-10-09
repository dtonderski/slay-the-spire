# A0 run training

The sole training entry point is **`rl/train.py`**. `--task combat` (default) keeps
the existing combat loop. `--task run` dispatches to a separate implementation:

- `collector.py`: natural A0 runs, public macro decisions and frozen **sampled**
  combat (including escape potions); forced actions execute without actor loss.
- `model.py`: small complete-candidate macro scorer with a **new run-value head**.
  It uses deterministic hashed features of typed public observations, public
  current-act map history, previous noncombat action and revision-free action
  descriptors plus their resolved visible cards/offers/map destinations. The
  policy starts uniform and the fresh critic starts at zero. This is a correctness
  baseline: hashing has collisions and is
  **not** the planned lossless numeric run encoder / set-graph architecture.
- `trainer.py`: completed-episode Monte Carlo actor-critic (`gamma=1`), on-policy
  public-input recomputation checks, fixed held-out evaluation, logs/checkpoints.
- `rewards.py`: declared terminal reward components and outcome-based success.
- `metrics.py`: accepted healing decisions by HP band and elite share of map entries.
- `contracts.py`: explicit schema-8 run outcomes and screen-based ownership.
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

## Environment limitations

Known failure classes include Prismatic Shard and Courier purchase successors,
and Headbutt selection completion leaving a won combat without a legal next
run action. These have simulator-only reproductions; they are not real-game
parity findings. The default **aborts** on simulator errors or incomplete training
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
`--max-consecutive-skips` (default 8) stops a repeatedly failing campaign.

Use `--max-hours 8 --compress-journals` for a bounded overnight experiment.
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
original training job. It is never optimized by this trainer.

After an intentional simulator/source update, use `--warm-start /path/to/latest.pt`
for explicit **macro weights-only transfer** into a new run. Architecture, feature,
observation, reward, objective and frozen-combat contracts must match; native/source
hash changes are allowed and parent checkpoint/native hashes are recorded. The
optimizer, sampler and local iteration counters start fresh. This is not exact
resume and does not restore simulator states or unfinished episodes.

The unchanged `hashed` encoder retains feature version **2**. Weights-only warm
starts from #93 checkpoints interpret a missing `encoder` field as `hashed`.
Pre-review #141 checkpoints explicitly tagged `encoder=hashed, feature_version=3`
can also transfer weights to hashed version2; their parent encoder/version are
recorded. The opt-in `health` encoder remains version3. Unknown feature versions,
encoder changes and incompatible reward/model contracts still fail. These narrow
compatibility rules apply only to `--warm-start`; exact resume checks are unchanged.

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
- Public hash paths, immutable public map contributions and candidate facts have
  bounded caches. Cached entries contain no learned activations or hidden state;
  feature bucket values and collision accumulation order remain unchanged.
- The learner packs public inputs once, verifies all inference/recomputation
  pairs **before any backward**, then backpropagates chunks. Actor sums are still
  divided by episodes; value/entropy terms are still divided by macro decisions.
  There is one gradient clip and one optimizer step per complete episode batch.
- A failed action is never retried. Explicit continuation mode still quarantines
  the whole learner batch. Unlocalized collective numeric-export failures and
  model/infrastructure errors are fatal, not per-episode death labels. Open peer
  journals receive cancellation records. Cooperative stops finish in-flight
  decisions, launch no new episodes, and leave incomplete batches untrained.

For the serial reference path use
`--collection-width 1 --learner-batch-size 0`. Larger reductions can differ in
FP32 rounding; bitwise equality with the old serial learner is not promised.
Execution settings and source hashes are part of strict resume validation.
Changing execution mode requires a new identity; use compatible weights-only
warm start rather than bypassing resume checks. Default combat-task training is
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
workload-specific measurements, not learning-strength claims. All 448 CLI episode
setups and execution fingerprints matched. A CUDA split/resume check reproduced
the uninterrupted 16-update model, optimizer and saved RNG states exactly.

`train/collection_seconds`, `train/update_seconds` and
`validation/collection_seconds` expose phase timings. Typed projection and public
feature assembly remain CPU costs. Native `numeric_steps` is deliberately not used:
its non-atomic batch failure contract needs trustworthy per-state settlement
accounting before replacing individually journaled transitions.

The bounded comparison tool is [`benchmark_run_training.py`](../tools/benchmark_run_training.py).
It is not an alternative training entry point. It resets fixed macro weights per
trial, pins combat/native/source hashes, records complete-case coverage and
execution fingerprints, and skips the entire optimizer update on any unresolved
episode. Example (use a new directory):

```bash
PYTHONPATH=rl uv run --project rl --no-sync python rl/tools/benchmark_run_training.py \
  --profile natural --macro-checkpoint /path/to/macro/latest.pt \
  --combat-checkpoint /path/to/combat/latest.pt --seeds /path/to/seeds.json \
  --episodes 64 --trials 3 --width 64 --learner-batch-size 128 --out /path/to/new-benchmark
```

Both benchmark profiles use binary Act-1 completion targets. For
`--profile campfire`, also supply `--root-manifest`; initial reconstruction is included in
cold timing. Run comparisons sequentially on the same hardware, retain every
journal, and report warm/cold results separately. A serial baseline can be run
under its own checkout's `PYTHONPATH` with `--width 1 --learner-batch-size 0`.

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
