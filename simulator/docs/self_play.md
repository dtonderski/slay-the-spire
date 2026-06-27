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
py -3.14 -m sts.self_play run --start map_fixture --random-seed 7 --max-steps 40 --output target\selfplay-map.jsonl
py -3.14 -m sts.self_play verify target\selfplay-map.jsonl
```

The trace can include potions when the simulator run state exposes them. For
example, the current map fixture can buy a Fire potion from the shop; the potion
inventory is recorded in every step summary and full snapshot.

## Current Blocker

True seed-start self-play is not available yet. The Python API currently raises
an explicit unsupported-start error for:

```powershell
py -3.14 -m sts.self_play run --start seed --seed TEST --output target\selfplay-seed.jsonl
```

The runner records that unsupported start as a metadata-only trace instead of
pretending the run began. To play real seeded runs end-to-end, `OmniRunEnv` needs
a seed-start constructor exposed at the Python boundary. A placeholder generated
map fixture exists in `sts_core`, but it is not target-game seed parity.
