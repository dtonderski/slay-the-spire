# Simulator Self-Play

## Purpose

Self-play generates simulator-native JSONL traces for policy iteration and
future training data. Each step records the full before/after simulator
snapshots, legal actions, chosen action, policy metadata, transition hashes, and
compact summaries including potion inventory.

Combat uses the current omniscient portfolio search policy. Non-combat choices
use `random_viable_v1`: shuffle exact legal actions with a fixed RNG seed, then
pick the first action that does not immediately enter an unsupported no-action
state.

## Commands

```powershell
$env:PYTHONPATH = "$PWD\python"
py -3.14 -m sts.self_play run --start seed --seed TEST --random-seed 7 --max-steps 40 --output target\selfplay-seed.jsonl
py -3.14 -m sts.self_play verify target\selfplay-seed.jsonl
```

The trace can include potions when the simulator run state exposes them. For
example, `--start seed --seed 3 --random-seed 4 --max-steps 40` buys a potion
from the placeholder seeded shop; the potion inventory is recorded in every step
summary and full snapshot.

## Seed Fidelity

Seeded starts currently use the simulator-only placeholder generated map
fixture. They are deterministic and replayable, but they are not target-game
seed parity.

```powershell
py -3.14 -m sts.self_play run --start seed --seed TEST --output target\selfplay-seed.jsonl
```

To play target-game seeded runs end-to-end, `OmniRunEnv` still needs a
source-backed seed-start constructor. Until then, self-play traces should keep
`source = "sim_selfplay"` and should not be mixed with real-game parity traces.
