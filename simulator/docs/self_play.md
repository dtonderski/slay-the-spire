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
uv run maturin develop --release
uv run python -m sts.self_play run --start seed --seed TEST --random-seed 7 --max-steps 40 --output target\selfplay-seed.jsonl
uv run python -m sts.self_play verify target\selfplay-seed.jsonl
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
uv run python -m sts.self_play run --start seed --seed TEST --output target\selfplay-seed.jsonl
```

To play target-game seeded runs end-to-end, `OmniRunEnv` still needs a
source-backed seed-start constructor. Until then, self-play traces should keep
`source = "sim_selfplay"` and should not be mixed with real-game parity traces.

## Corpus Generation

Use `batch` to generate a simulator-only training/evaluation corpus. The batch
command runs one trace per seed, verifies each trace, and writes
`index.json` plus trace files under `traces/`.

```powershell
uv run python -m sts.self_play batch --output-dir target\selfplay-corpus --seeds 1..100 --random-seed 1000 --max-steps 200
```

The index is labeled:

- `source = "sim_selfplay_corpus"`
- `parity = "non_parity_simulator_only"`

That label is intentional. This corpus is useful for search iteration and
regression tests, but it is not evidence that the simulator matches the target
game.

## Trace-Based Search Eval

Use `eval` to compare combat search candidates from exact combat states recorded
inside the corpus traces. The original trace is the fixed eval dataset; candidate
policies are rolled forward from each recorded combat snapshot.

```powershell
uv run python -m sts.self_play eval --corpus-dir target\selfplay-corpus --split eval --max-roots 64 --max-actions 40 --output target\selfplay-corpus\eval.json
```

Restrict potion use during search/eval with `--allowed-potions`. Use a
comma-separated list, `all`/`*` for no restriction, or `none` to forbid potion
use:

```powershell
uv run python -m sts.self_play eval --corpus-dir target\selfplay-corpus --allowed-potions "Fire,Block,FruitJuice"
uv run python -m sts.self_play eval --corpus-dir target\selfplay-corpus --allowed-potions none
```

The eval report includes potion metadata:

- `potion_roots`: recorded combat roots where the run had potions.
- `potion_action_roots`: recorded combat roots where potion actions were legal.
- `allowed_potion_roots`: recorded combat roots where at least one legal potion
  action survived the allowlist.
- per-episode `potion_count`, `legal_action_kinds`, and `has_potion_actions`.

## CommunicationMod Trace Boundary

`eval` consumes simulator self-play traces because those records include
`initial_snapshot_json` and per-step `before_snapshot_json`. Long
CommunicationMod traces contain observed game states instead. They can have many
potions, but they are not directly replayable as simulator roots until a
state-reconstruction adapter emits simulator snapshots.

Use `real-trace-report` to inspect that boundary:

```powershell
uv run python -m sts.self_play real-trace-report --trace ..\verification\corpus\communication_mod\trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl
```

The report counts observed combat/potion combat states and explains whether
root extraction is blocked by missing simulator snapshots.

To try converting a real trace into simulator-backed roots, run trace-guided
replay:

```powershell
uv run python -m sts.self_play replay-real-trace --trace ..\verification\corpus\communication_mod\trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl --output target\trace-guided\manual01-replayed.jsonl --report-output target\trace-guided\manual01-report.json
```

This starts `OmniRunEnv` from the trace `START` command, maps supported
CommunicationMod commands onto exact simulator actions, verifies the observed
summary before each command, and writes a normal replayable JSONL prefix. If the
trace reaches combat while still in sync, the generated JSONL can be passed to
`eval` like any other simulator trace.

Current limitation: the MANUAL01 clean-prefix trace stops at Neow before any
combat roots because the Python seed-start path is still the local placeholder
map path. The replay report should show `observed_simulator_divergence` with an
observed `event` phase and simulator `map` phase.
