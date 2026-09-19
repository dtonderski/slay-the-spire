# Synthetic combat training

One training entry point: **`train.py`**. One model input format and one
rollout implementation: public numeric observation batches. No legacy trainers,
epoch-based root collection, typed-model fallback, or alternate loss implementation.

## Read the code in this order

1. `model.py`: encode the observation and each legal action, then dot-product score them.
2. `train.py::play_combats`: clone roots, sample actions, step the simulator, collect decisions.
3. `trajectories.py::Trajectories.losses`: policy loss and entropy regularization.
4. `train.py::train_batch`: collect → loss → backward → optimizer step.
5. `train.py::main`: fresh batches, evaluation, logging, checkpoints.

Rollout, update, evaluation, and orchestration are all in `train.py`.
The other training files have been deleted, not retained as wrappers.

## Model

```text
public observation tables → feature encoders → tokens → transformer summary
                                                        ↓
                                                policy query [B, 64]
legal action features → action vectors [B, A, 64]         ↓
                            └────────────────── dot products → logits [B, A]
```

Padding actions get `-inf` logits. `action_dim` is an embedding width, not an action
count. `CombatValueModel` adds policy and scalar value heads to the shared state representation. `observation_encoder.py` builds tokens;
`encoders/` contains only the numeric feature path. Old policy-only checkpoints are not directly interchangeable with policy/value checkpoints.

Policies receive public observations only—not constructor seeds, hidden draw
order, simulator debug state, or search results. Some public information is not
yet encoded; see `docs/fair_observation_hidden_state_audit.md`.

## Run

From `rl/`, with the local fitted distributions and frozen validation file available:

```bash
uv run python train.py --run-id my-run \
  --distributions ../data/slaythedata/loadout-a0-v3/fit.json \
  --validation-manifest ../data/slaythedata/validation-a0-v2.json \
  --device cuda --batch-size 256 --updates 10000 --eval-every 100 \
  --value-coef 0.1 \
  --continue-on-simulator-error
```

Every update samples fresh independent A0 roots. Reward is terminal HP divided by
starting max HP; defeat gives zero. Truncations are excluded from training losses,
not treated as defeats. The objective averages summed decision losses over completed
episodes, with entropy coefficient `--entropy-coef` (default 0.01).

The policy advantage is final return minus the current state's value prediction.
The policy loss detaches this advantage; squared value error trains the value head.
`--value-coef` weights that error (default 0.1). Both objectives sum decisions within
fights and average over completed fights. There is no scalar/EMA reward baseline.
Random/beam evaluation references are not learning baselines.

## Fixed evaluation and references

`validation_set.py` creates immutable synthetic specifications with HP installed
**before** combat-start effects. Training evaluates only the 1,024 main cases
(10–100% pre-entry HP). The existing file also contains 352 deliberately low-HP
stress cases; these are preserved but no longer evaluated by `train.py`.
All frozen seeds remain excluded from training, including the unused stress cases.
Max HP/loadouts are drawn from historical fitted marginals, not reconstructed runs.
Healing at combat start may raise HP above the pre-entry band.

Files pin the native binary hash and initial public-observation hashes. Mismatches
fail instead of repairing state. They also pin policy sampling seeds/repeats and
decision limits. Creation refuses to overwrite existing files. Data is Git-ignored;
back it up separately. Historical datasets/logs/checkpoints are not deleted by cleanup.

Main-set outcomes are logged under `val_main/`. Decision statistics are collected
only for training, in `Trajectories`, rather than duplicated in each episode.
Random and **privileged** beam references for the main set are saved in
`baselines.json` and logged throughout training. Beam defaults to width 64
and 10,000 transitions per root; it is budget-limited, not an optimal upper bound.
`--reference-run wandb/PRIOR_RUN` reuses references only when input hashes and
relevant evaluation budgets match.

Checkpoints (`wandb/RUN_ID/latest.pt`) contain policy/value model, optimizer, and
RNG states. Automatic resume is not implemented. Use `--wandb-mode disabled` for
local smoke tests; no training starts merely by importing modules.

The active validation artifact is `validation-a0-v2.json`, revalidated after PR #46.
All 1,376 specifications, initial public-observation hashes, and evaluation settings
are identical to v1; only the native pin and revalidation provenance changed.
Do not reuse pre-PR46 random/beam caches: their native and dataset hashes differ.
They must be recomputed on the new simulator.

`--max-hours 8` bounds the training/validation loop by wall time; a current batch
and final validation can extend slightly beyond the limit. Initial validation is
outside that budget. Periodic checkpoints are retained as `checkpoint-NNNNNNNN.pt`
in addition to the atomically replaced `latest.pt`.

## Errors and diagnostics

Training simulator failures propagate to a single diagnostic handler. With
`--continue-on-simulator-error`, it saves specs/attempted prefixes and discards the
whole batch without an optimizer step. Partially advanced clones are never retried.
Validation errors are recorded as unavailable attempts with coverage counts, not
losses. Unexpected infrastructure/model errors still abort.

`beam_search.py` is used only for the privileged evaluation reference.

## Checks

```bash
uv run --no-sync python -m unittest discover -s tests -q
uv run --no-sync ty check model.py observation_encoder.py encoders train.py \
  trajectories.py validation_set.py
```

Tests cover the supported numeric path, loss/gradient behavior, error handling,
frozen inputs and reference logging. Unit/synthetic tests do not establish real-game
parity; simulator changes require the repository's reviewed trace-corpus checks.
