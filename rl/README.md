# Reinforcement Learning

This area contains RL-facing design documentation and surviving experimental
notebooks. Its dependency on the simulator is deliberately one-way:

```text
rl -> simulator
```

Code under `simulator/` must never import or depend on `rl/`. RL work should use
only the simulator's public fair observations and decision-local actions; it
must not consume privileged serialized state as a policy observation.

## Layout

- `model.py`: batch-only policy composition.
- `observation_encoder.py`: input-slice assembly, transformer, and summary-token query.
- `encoders/`: one file per input type, containing its tensorizer and encoder:
  - `cards.py`, `enemies.py`, `player.py`, `potions.py`, `relics.py`: raw features and token projections.
  - `selection.py`: selection context and card/selected-bit tokens.
  - `actions.py`: feature gathering, action-specific MLPs, and learned no-object vectors.

- `examples/fair_simulator.py`: runnable fair API walkthrough; start with
  [`examples/README.md`](examples/README.md).

- `docs/fair_combat_api_design.md`: fair observation and choice boundary.
- `docs/fair_observation_hidden_state_audit.md`: hidden-state exposure audit.
- `notebooks/combat_rl_playground.ipynb`: surviving RL experiment notebook.

The notebook is retained as in-progress research material. Its current imports
refer to RL Python modules and optional notebook dependencies that are not in
the present `sts_sim` package, so it is not runnable from the base simulator
environment. Do not restore those deleted components implicitly.

Build and validate the simulator first using
[`../simulator/README.md`](../simulator/README.md). The fair Rust boundary lives
in `simulator/crates/sts_env`, and Python policy code installs
`simulator/python` as its upstream package. RL dependencies must never flow back
into the simulator workspace.

## Combat-only task and privileged reference

Training, random evaluation, root collection, and beam search exclude **Smoke Bomb
use** through `combat_task.py`. The inventory/observation and simulator rules stay
unchanged; the filter is recomputed at each decision, including after Entropic
Brew. Existing saved action prefixes are not rewritten.

Completed fights score **terminal HP / starting max HP**, with defeat scoring zero.
Unfinished fights have no terminal reward, rather than an invented loss.

```bash
cd rl
uv run python beam_search.py --seed HUMAN1 --width 64 --max-decisions 128 --max-transitions 10000
```

`beam_search.py` is a deliberately small **privileged** reference built from scratch.
It explores clones of the actual state, so its simulated futures exploit actual
hidden draw order and RNG. Never pass that state or search outputs into the fair
policy as observations. It returns the best completed action sequence found,
its reward, transition count, and whether a search limit was reached. No completed
path means `reward: null`, not a defeat. The root is not modified.

Only terminal return ranks completed paths—no potion or gold bonuses. To prune
unfinished paths, the beam scores HP minus estimated unblocked incoming damage
(5×), enemy HP (1.75×), enemy block (0.25×), and living enemies (12×), plus
useful block (0.75×) and energy (0.25×). Visible multi-hit intents are included;
hidden intents contribute no damage estimate. Card damage values are not exposed
by this API, so there is no hand-damage term. The beam reserves one path per
opening action when width permits, then fills remaining slots by score.
This heuristic, beam pruning, and finite budget make this
an approximate reference, **not an upper bound or proof of optimal play**. There
is no deduplication, caching, or replanning machinery. Search errors propagate.

### Baselines during frozen-root training

`train_roots.py` evaluates random and privileged beam references on the same frozen
validation roots before training. Beam runs once per root (deterministic); random
uses `--eval-repeats`. Set search effort with `--beam-width` (default 64),
`--beam-transitions` (default 10000), and the shared `--max-decisions` limit.

W&B logs `random_val/*` and `beam_val/*` initially and repeats these cached
reference scores at every periodic/final policy evaluation. Compare their
`mean_return_completed`, `mean_hp_completed`, and `win_rate_completed` with the
policy's `val/*` metrics. **Check coverage first:** beam `unfinished` and `errors`
are separate from defeats; completed-only averages exclude those roots and are
not directly comparable if coverage differs. `found_win_rate_all_roots` reports
wins found divided by all validation roots, not a claim that unresolved fights
are losses. Training, policy validation (initial/periodic/final), random, and beam
also log `mean_hp_lost_completed`: starting HP minus terminal HP, averaged over
completed fights. This is **net** loss including healing (negative means net HP
gain), not total damage taken. Defeats end at zero HP; truncations and search errors
are excluded. Beam per-root records additionally contain `starting_hp` and `hp_lost`. Search transitions, budget/depth-limit hits, and total reference
runtime are logged too.

Per-root beam outcomes and replay-verified native action indices are saved in
`wandb/<run-id>/baselines.json`. Search failures retain error details and print
tracebacks. Collection and initial reference evaluation do not consume `--hours`;
that clock starts when training begins. Reference computation adds startup time
but is not repeated during training.

### Entropy and temporary synthetic Act 1 curriculum

Both trainers accept `--entropy-coef` (default `0.01`; `0` restores plain REINFORCE).
Completed-episode loss is `-return * sum(log_probs) - coefficient * sum(entropies)`,
then averaged across completed episodes. Truncations contribute neither term.
The task reward and evaluation remain unchanged. Summed entropy can favor longer
trajectories; monitor truncations and held-out performance rather than maximizing
entropy. `policy_entropy`, `normalized_policy_entropy` (divided by log candidate
count), `mean_max_action_probability`, and `mean_action_count` exclude forced
choices. These are raw candidate diagnostics, not equivalent-action-group entropy.
The trainers also log `policy_loss`, `entropy_bonus`, and combined `loss`.

For the explicitly synthetic overnight experiment:

```bash
cd rl
uv run python train_roots.py --run-id "act1-entropy-$(date +%Y%m%d-%H%M%S)" \
  --synthetic-act1 --train-seeds 500 --val-seeds 50 --batch-size 128 \
  --device cuda --entropy-coef 0.01 --hours 8
```

`--synthetic-act1` overrides `--floors`: collection starts at 10000/10000 HP,
uses random allowed actions, and stops after the Act 1 boss (before boss rewards).
Every encountered combat is cloned into a 100/100 HP scenario; the collection
run retains its own HP. Train/validation seeds are disjoint, and roots remain
fixed throughout training. `roots.json` records accepted prefixes and both HP
construction parameters: reconstruct using `State.new_synthetic`, replay the
prefix, then call `synthetic_combat_root(100)`. Never load these prefixes as
ordinary 80-HP runs. This is not a real-game trace or representative normal-HP
evaluation. Already-applied combat-start effects are not recalculated. Collection
deaths, cutoffs, and errors remain reported; high HP does not guarantee survival.

### Simulator-step failures during frozen-root training

The CLI logs a **CRITICAL** warning and continues if a native simulator step
raises `ValueError`. It discards the **entire affected batch**, including any
already completed episodes: numeric steps are not batch-atomic, so partially
advanced clones are never retried. Frozen roots remain unchanged and available
for subsequent epochs. No gradient, reward, defeat, or truncation is assigned to
the discarded batch. This reduces training coverage and can bias which episodes
contribute; it is not a simulator fix or evidence of parity.

W&B logs `train/simulator_error_batches`, `train/discarded_episodes`, their
`_total` counterparts, and `train/optimizer_step=0` for failed batches. The run's
`simulator_errors/update-XXXXXX.json` saves the traceback, dataset root indices,
seed/act/floor, and native action prefixes (including the attempted current
step, whose acceptance may be unknown). Reconstruct from the immutable
`roots.json` plus those prefixes, not from partially advanced clones. Diagnostic
write failures remain fatal rather than silently losing evidence.

Only errors raised at simulator-step boundaries are caught. Model errors, CUDA
failures, non-finite losses/gradients, and other programming errors still stop
training. Direct `train_batch` calls remain fail-fast unless supplied an
`error_path`; validation keeps its existing error accounting.

### Extending the synthetic curriculum through Act 4

Replace `--synthetic-act1` with `--synthetic-act4` to collect all four acts:

```bash
cd rl
uv run python train_roots.py --run-id "act4-entropy-$(date +%Y%m%d-%H%M%S)" \
  --synthetic-act4 --numeric-observations --train-seeds 500 --val-seeds 50 \
  --batch-size 1024 --device cuda --entropy-coef 0.01 --hours 8
```

The new flag enables the simulator's existing final-act profile at initial run
creation. It does **not** grant keys: the collector follows publicly visible
routes to the burning elite, takes legal key choices, and opens ordinary chests
until it obtains the sapphire key. Other decisions remain random. Victory screens
with a legal Proceed are traversed, including the Act 3-to-Act 4 transition.
Collection still starts at 10000/10000 and freezes roots at 100/100. `--floors`
is ignored; death, run completion, unsupported transitions, and the 5000-action
cap terminate a collection seed without replacement. Coverage is not balanced or
guaranteed: W&B reports `train_roots_act_1` through `train_roots_act_4` and matching
validation counts. Training fails explicitly if either split has no Act 4 roots.

New manifests record each root's act, the final-act profile, key-priority policy,
and failed action details. Reconstruct Act 4 runs using
`State.new_synthetic(seed, final_act=True)` before replaying their prefixes; older
Act 1 artifacts keep the default `final_act=False`. Existing frozen datasets are
not rewritten or extended in place. Later fights/decks increase GPU memory use:
three 1024-episode CUDA updates on a mixed-act probe passed at about 9.5 GiB peak
tensor allocation, but this does not guarantee headroom for every future batch.
The first Colosseum fight ends at its event dialog; rollout and beam evaluation
score that fight before the separate choice to enter its second fight.

## Relic and potion identity embeddings

Run Python from `rl/` with `uv run`. Each identity-bearing encoder creates its
own embedding table and projection. Create each encoder once, not per call:

```python
from encoders.relics import RelicEncoder
from encoders.potions import PotionEncoder

relic_encoder = RelicEncoder()
potion_encoder = PotionEncoder()

# Each returns lists of raw feature tensors and token tensors, one per observation:
relic_features, relic_tokens = relic_encoder([observation.context.relics])
potion_features, potion_tokens = potion_encoder([observation.context.potion_slots])
```

Tensorization lives inside the classes. Batch-first `forward` calls flatten input
rows, project once, then split into per-observation lists without adding padding.

Outputs preserve input order. Empty potion slots have their own index (`0`),
and empty inputs return zero rows. Each relic vector concatenates three raw,
alphabetically key-sorted counter slots after its identity embedding; unused slots are
zero. Mappings use the current catalog; checkpoint compatibility is not implemented.

`PlayerEncoder.tensorize(observation)` returns a vector ordered as
`[hp, max_hp, block, energy, max_energy, gold, power amounts...]`.
HP/max HP/block are divided by 100, energy/max energy by 10, and gold by 1000,
without clipping. Power amounts remain raw and use `POWER_TO_INDEX`, with zero
for absent powers. Features follow the encoder's device and dtype.

`CardEncoder.tensorize(cards)` appends 11 raw state values to each card
embedding (27 features with the default 16-dimensional table): cost, upgrade
level, cost-modified/reset flags, temporary flag, Rampage/Ritual Dagger/Windmill/
Steam Barrier bonuses, underlying combat cost, and its presence bit. One `CardEncoder` owns the embedding and projection
shared across all piles and selection cards. For the hand, pass `tuple(entry.card for entry in observation.screen.hand)`;
for a pile, pass its `.cards`. Draw `known_positions` remain separate information.
An optional underlying combat cost has a presence bit; the other dynamic bonuses
default to zero. Bottled status is omitted for this combat v0.

`EnemyEncoder.tensorize(monsters, cards)` takes the shared `CardEncoder`
for Stasis-held cards. Absent held cards have zero features
and a separate presence flag;
slime size distinguishes none/small/medium/large, and stolen gold is divided by
1000. Dead/escaped entries stay in input slot order.

`tensorize_actions` returns a tuple of `(action_kind, raw_features)` pairs in
legal-candidate order. Feature vectors have different widths by kind: play/use
concatenate the referenced object, enemy target (zeros if absent), and target
presence; discard uses potion features; selection toggle/choose uses option
features. End-turn, confirm, confirm-without-retrieval, and skip have empty
vectors, distinguished by their kind. Unsupported kinds raise.

There are no action layers in the tensorizer. It only gathers/concatenates
existing features, preserving gradients to shared embeddings. `ActionEncoder`
in `encoders/actions.py` accepts a list of candidate tuples and the observation
encoder's list of reusable feature dictionaries:
`action_vectors = self.action_encoder(action_batches, features)`.
It returns one unpadded `[n_actions_i, action_dim]` tensor per observation (default 64).
Each feature-bearing kind
has its own `Linear → ReLU → Linear` MLP; no-object kinds have separate one-entry
embeddings. Feature widths are fixed by the tensorizers.
Batched action assembly builds validated integer slot indices first, then gathers
all objects/targets of a kind at once from packed feature tables. Untargeted actions
use a shared zero target row plus a false presence flag. Each kind is encoded once
across the batch, then vectors are restored to their original candidate positions
and split by observation. The standalone `tensorize_actions` helper remains a
row-wise reference; the batched forward path does not call it. Empty candidate tuples
return `[0, action_dim]`; action and feature batch lengths must match.

```python
from encoders.actions import tensorize_actions

# Use the existing ObservationEncoder instance and its shared card encoder.
selection_features, context_tokens, option_tokens = encoder.selection(
    [observation.screen.selection], encoder.cards
)
action_inputs = tensorize_actions(
    decision.actions, hand_features, potion_features, enemy_features,
    selection_features=selection_features[0],
)
```

Selection context is a one-hot kind vector (including no selection). Option rows
are shared card features plus an already-selected bit, in visible option order,
not hand/pile order. No selection returns an empty option matrix. The selection
slice feeds both context and option tokens to the observation transformer.

## Observation encoder

`ObservationEncoder` projects each group to `d_model` (default 64), adds learned
group/location embeddings, and applies a two-layer, four-head transformer.
Its learned summary tokens are projected to `[batch, action_dim]` (default 64).
There is no positional encoding or dropout in this initial version.

It accepts a list of raw `CombatObservation` objects. Slices
in `encoders/` handle player, cards, enemies, relics, potions, and selection.
Each slice tensorizes its raw objects and projects features into tokens. One
card projection is reused for every pile and selection cards; selection adds a
selected-bit projection. Location embeddings distinguish hand/draw/discard/etc.

The card, enemy, potion, and relic encoders each own one identity table. Stasis
and selection receive the shared `CardEncoder` when called, without registering
a duplicate instance. The observation encoder returns batched queries
and a list of raw hand/enemy/potion/selection feature dictionaries. Actions reuse those rows **before**
observation projection, so no card/potion/enemy tensorization is repeated for
action scoring. No extra shared MLP is added.

```python
from model import CombatModel

model = CombatModel()  # Create once; default d_model=64 and action_dim=64.
# Given a current combat decision from State.decision():
logits, valid_actions = model([decision.observation], [decision.actions])  # [1, n_actions]
probabilities = logits.softmax(dim=-1)
```

The model computes observation features once, encodes the supplied candidates,
and returns `action_vectors @ query` in candidate order. Pass actions from the
same decision; noncombat kinds remain unsupported.

`CombatModel.forward` accepts batches only. Training batches the active
simulators' decisions; single-combat evaluation uses a batch of one.
Empty groups may have zero tokens. Keep dead enemies and empty
potion slots; the summary token is always present.

This consumes the existing tensorized features, not every public observation
field. Known draw positions and other not-yet-tensorized context are not added
implicitly. This is not yet a complete encoding of every gameplay-relevant field.

## Observation batching: concatenate first, pad once

```python
encoder = model.observation_encoder
raw, tokens = encoder.cards([cards_a, cards_b])
# raw:    [Tensor[n_cards_a, 27], Tensor[n_cards_b, 27]]
# tokens: [Tensor[n_cards_a, 64], Tensor[n_cards_b, 64]]

queries, features = encoder([observation_a, observation_b])
# queries: [2, action_dim]
# features[i]["hand"]: [n_hand_i, 27], with no padding

# To inspect the assembled transformer input without running attention:
tokens, padding_mask, features = encoder.prepare_batch([observation_a, observation_b])
# tokens: [2, max_total_tokens, d_model]
# padding_mask: [2, max_total_tokens], True only for added padding
```

Each slice returns variable-length lists; projections operate on flattened rows
across the batch. `ObservationEncoder` adds group embeddings and concatenates
all real groups within each observation, with the summary token first. Only then
are the complete sequences padded. Internally, tokens are packed by group and each
group embedding is added once; an integer lookup table gathers the complete padded
sequences in one operation. This avoids per-observation concatenation/addition
and padding graphs without changing token order or the list-based slice interface. Player and selection context contribute one
token each. An empty observation list raises `ValueError`.

For example, 3 cards + 3 enemies and 5 cards + 1 enemy each need 6 tokens, not
8 tokens from independently padding both groups. This count excludes the other
groups and summary token. Masks prevent padding from affecting real tokens.
Raw action features retain per-observation slot order and gradients; no action
can reference another observation's rows through the batched model.

## Batched action scoring

```python
import torch

logits, valid_actions = model(
    [decision_a.observation, decision_b.observation],
    [decision_a.actions, decision_b.actions],
)
# Both: [2, max_actions]. True means a real candidate (not padding).
indices = torch.distributions.Categorical(logits=logits).sample()  # [2]
# Step each simulator with its original decision.actions[indices[i].item()].
```

Action references are resolved within their own observation before grouping by
kind. Scoring pads the encoded candidate vectors and sets invalid logits to
`-inf`, so padding has zero probability. `forward` rejects empty batches,
mismatched batch lengths, and observations with no candidates; terminal episodes
must be handled outside the policy.

## Batched rollouts

`play_combats(roots, model, max_decisions=..., training=True, rng=...)` clones a
fixed group of initial roots. Each round scores all active decisions in one
policy call, then steps the simulators sequentially with their own actions.
Completed/truncated episodes drop out; no new roots refill the group. Results
stay in original root order, with separate rewards and log-probability histories.
Outcomes are checked before cutoffs, including victory on the final allowed action.

Training takes one backward pass on the mean completed-episode REINFORCE loss.
Truncations contribute neither loss nor denominator; all-truncated multi-root
batches skip the optimizer. Graphs are shared across episodes and retained until
the whole group finishes, so memory grows with batch size and combat length.
There is no multiprocessing, baseline, entropy bonus, or reward change.
Evaluation keeps its fixed per-episode sampling seeds and runs batches of one.

Quick training smoke test (CPU or `--device cuda`):

```bash
cd rl
uv run python train.py --updates 2 --episodes-per-update 4 --eval-episodes 2 --wandb-mode disabled
```

## Direct numeric observation path

Training CLIs default to `--numeric-observations`; use `--no-numeric-observations`
for the original typed reference. Programmatic `play_combats`/`train_batch` retain
`numeric=False` by default; pass `numeric=True` explicitly. Evaluation remains on
the typed path. Model parameters, rewards, and optimizer rules are unchanged.

Rust batches existing **fair** decisions into raw signed-integer tables and a
public-string dictionary. There is no observation JSON transport, typed Python
observation construction, or intermediate view format on this path. Python makes
read-only NumPy views; each encoder owns categorical remapping, normalization,
embeddings, and projections. Groups stay flat until one vectorized layout gathers
complete padded sequences. Only action-feature rows are split by observation.
The simulator has no torch/NumPy/RL dependency and does not export hidden state.
See the [numeric API contract](../simulator/docs/python_api.md#numeric-combat-batches).

### Correctness argument and evidence

For the fields consumed by the policy, the required relation is
`numeric_encode(public_export(state)) == typed_encode(fair_observation(state))`.
This is a refinement argument, not a machine-checked formal proof:

1. The exporter consumes `FairDecision`, never authoritative state. Batch methods
   call the same `FairEnvironment.decision/step` as the typed API, with the same
   revision-bound actions. Export does not draw RNG or alter state.
2. Integer values, optional-presence flags, categorical public keys, and row order
   are preserved. Batch-local dictionary numbers are mapped back through the
   encoder vocabularies; they are never numeric policy features. Dead/escaped
   enemies and empty potion slots are retained.
3. Each encoder performs the same field selection, scaling, one-hot encoding,
   counter ordering, and shared-card lookup. Group layout preserves summary/group
   order and pads only complete sequences. Owner offsets cannot cross observations.
4. Therefore the policy inputs and mathematical gradients agree. With matching
   sampling inputs, the unchanged transition rule gives the same next state;
   this extends inductively through a rollout. Floating-point accumulation order
   can differ, so tests check gradients with explicit tolerances rather than
   claiming universal bitwise equality.

`tests/numeric_reference.py` independently extracts tables from the original typed
API. `tests/test_numeric_observations.py` checks native trajectories against it,
RNG noninterference, stale-action rejection, buffer lifetime/read-only ownership,
CPU/CUDA float32/float64 features/masks/logits/gradients, category-code permutation,
Stasis/selection/optional-zero fixtures, and escape at the truncation boundary.
Rust unit tests separately check optional values and raw Stasis/selection references.
Frozen multi-root benchmark trials also matched all logged metrics, including loss.
These tests establish implementation equivalence, not real-game parity by themselves.
The reviewed corpus replay passed 433/433 traces (642,896 actions).

## Profiling training batch sizes

```bash
cd rl
uv run python profile_batches.py --batch-sizes 128 256 512 1024 2048
```

CUDA-only multi-root probe: one full-update warmup per size, then three timed
updates. Additional seeds supply distinct roots; batches are never silently capped
or filled with duplicates. Each measurement starts from identical policy weights
and reset warmed Adam buffers; collection/setup/reset and W&B are excluded.
End-to-end decisions/second includes simulation, public export, feature assembly,
policy, sampling, backward, optimizer, and cleanup.

For an A/B comparison, pass the **same existing profiling pool** using
`--training-manifest <profiling-pool/roots.json>` to both runs, together with its
`--held-out-manifest <original-training-run/roots.json>`. Give each run a fresh
`--output-dir`; add `--numeric-observations` only to the numeric run. Without a
fixed pool, collection grows the dataset during a sweep and small batches from
separate sweeps need not select the same roots.

A separate synchronized pass reports exclusive phase wall times, splitting
native-step/wrapper work, decision getters/mapping, Python decoding, observation
feature construction/projection, transformer, action encoding, and backward.
Device synchronization changes overlap: these diagnostic times include waits,
not pure GPU kernel time. Use the uninstrumented trials for throughput.
`--warmups`, `--repeats`, `--max-decisions`, and `--output-dir` are configurable.
Each size runs in a fresh process; the sweep stops at the first failure, including
CUDA OOM. JSON results, root identities, and worker logs are saved incrementally.
`profile_observations.py --manifest <profiling-pool/roots.json> --output <file.json>`
separately measures export and observation preparation with paired randomized order.

On the RTX 5080, the numeric path measured about 7,793 / 8,996 / 10,284 decisions/s
at batches 512 / 2,048 / 4,096, versus 2,151 / 2,113 / 2,151 for the typed reference.
Batch 5,120 OOMed. At 2,048 roots, isolated export dropped from 474 ms to 14.3 ms
(~97%), and preparation from 124 ms to 11.4 ms (~91%). These are early-floor,
initial-policy measurements, not guarantees for later policies. Artifacts are in
`wandb/numeric-final-{reference,reference-large,direct}/` and `wandb/numeric-micro.json`.

## Multi-root overnight experiment

`train_roots.py --run-id <unique-name>` defaults to eight hours on CPU: 50 seeds
split 40/10 **before** collecting combats within the first ten floors at A0.
The fixed uniform-random legal collector retains early deaths and reports any
5000-decision collection cutoffs. Ten floors do not imply ten combat roots.

Training shuffles all training roots each epoch and rolls out eight episodes
per optimizer update in one fixed group (`--batch-size`). `--device cuda` enables
GPU policy batches; simulators remain sequential on CPU. Rewards remain
terminal HP / starting max HP, with no baseline. Validation uses three stochastic
episodes per held-out root every 20 minutes, with fixed evaluation sampling seeds
and no updates to model weights. Random and initial-policy baselines use the same
validation roots. Episode cutoffs remain separate from terminal outcomes.

The run saves `rl/wandb/<run-id>/roots.json` (seed + accepted-action prefixes) and
`latest.pt` (model/optimizer weights and progress) every validation and at exit.
These are local experiment artifacts, not a cross-version checkpoint format or
a resume implementation. The deadline is checked between training batches;
final validation/save can extend past it. Metrics go to local W&B.

## Training v1 and local W&B

`train.py` runs REINFORCE on the first combat of one fixed seed. CPU is the
default; `--device cuda` moves the model and tensor construction to the GPU.
The simulator and observation decoding remain on CPU. `--episodes-per-update`
sets the rollout group size. The earlier unbatched loop was slower on the RTX
5080 than on CPU; batched throughput needs a fresh benchmark. Each episode
resets to that initial combat; reward is terminal HP divided by starting max HP,
with defeat worth zero. This is plain REINFORCE without a baseline; win rate is
logged independently of reward.
Decision-limit truncations are logged separately and excluded from updates.
Metrics compare random, initial, and final stochastic policies on that same
combat; this is a wiring/overfitting experiment, not a generalization result.

The local W&B container is `sts-wandb-local`, bound only to `127.0.0.1:8080`.
Its data lives in the Docker volume `sts-wandb-data` mounted at `/vol`, and it
restarts automatically unless explicitly stopped. To manage it:

```bash
docker stop sts-wandb-local
docker start sts-wandb-local
docker logs --tail 50 sts-wandb-local
```

Tailscale Serve proxies **https://sorry.tail76d105.ts.net/** (tailnet only) to
`http://127.0.0.1:8080`. The container advertises
`HOST=https://sorry.tail76d105.ts.net`. Check the proxy with `tailscale serve status`;
disable it with `tailscale serve --https=443 off`.

Open that HTTPS URL from a Tailscale-connected device, complete the local account
setup, and enter your W&B Server license at `/system-admin` if prompted.
Then authenticate on the training server using an API key
from **this local instance**, not W&B Cloud (do not put keys in source control):

```bash
cd rl
uv run wandb login --host http://localhost:8080 --relogin
uv run python train.py --updates 100
```

The trainer explicitly defaults `--wandb-base-url` to `http://localhost:8080`.
Its `online` mode means sending metrics to that server, not necessarily the cloud.
Use `--wandb-mode offline` to save logs locally without any server connection.
W&B client logs are under `rl/wandb/` and ignored by Git. Initial server setup
and authenticated run ingestion must be completed before online training.

Installed server image (pinned digest):
`wandb/local@sha256:2fe35cad6d731eb6be1f63c45ee9f34962414e9b67ab39837cd59e26bb550db9`.
Container recreation must retain `-e HOST=https://sorry.tail76d105.ts.net`, the
localhost-only port binding, and the existing data volume; do not
remove `sts-wandb-data` unless you intend to delete all server data.
