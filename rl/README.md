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

- `model.py`: single-decision policy composition.
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
in `encoders/actions.py` maps those inputs to `[n_actions, action_dim]` (default 64).
It accepts raw candidates and the observation encoder's reusable feature rows:
`action_vectors = self.action_encoder(actions, features)`. Each feature-bearing kind
has its own `Linear → ReLU → Linear` MLP; no-object kinds have separate one-entry
embeddings. Feature widths are fixed by the tensorizers.
The encoder preserves candidate order and returns `[0, action_dim]` for no actions.

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
logits = model(decision.observation, decision.actions)  # [n_actions]
probabilities = logits.softmax(dim=0)
```

The model computes observation features once, encodes the supplied candidates,
and returns `action_vectors @ query` in candidate order. Pass actions from the
same decision; noncombat kinds remain unsupported.

`CombatModel` and the trainer still handle one decision at a time, using the
batched observation encoder with a batch of one. Action batching is not yet
implemented. Empty groups may have zero tokens. Keep dead enemies and empty
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
are the complete sequences padded. Player and selection context contribute one
token each. An empty observation list raises `ValueError`.

For example, 3 cards + 3 enemies and 5 cards + 1 enemy each need 6 tokens, not
8 tokens from independently padding both groups. This count excludes the other
groups and summary token. Masks prevent padding from affecting real tokens.
Raw action features retain per-observation slot order and gradients; no action
can reference another observation's rows through the current single-decision model.

## Multi-root overnight experiment

`train_roots.py --run-id <unique-name>` defaults to eight hours on CPU: 50 seeds
split 40/10 **before** collecting combats within the first ten floors at A0.
The fixed uniform-random legal collector retains early deaths and reports any
5000-decision collection cutoffs. Ten floors do not imply ten combat roots.

Training shuffles all training roots each epoch and accumulates eight episodes
per optimizer update. This is not parallel or tensor batching. Rewards remain
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
The simulator and observation decoding remain on CPU. The current unbatched
loop benchmarked slower on the RTX 5080 than on CPU. Each episode
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
