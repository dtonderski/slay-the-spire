# Project Overview

## Objective

Build the strongest fair Ironclad A20 Heart player, backed by a deterministic
simulator whose fidelity is established with immutable real-game traces.

The current workspace separates mechanics from their consumers: `sts_core`
implements the simulator, `sts_env` owns the fair observation/action boundary,
`sts_verify` replays CommunicationMod traces, and `py_sts` exposes the Python
interface. `rl/` contains synthetic combat training, evaluation, and a combat
explorer. Search and learning remain consumer responsibilities; `simulator`
must not depend on `rl`.

See the [knowledge index](docs/README.md) for current contracts, research
proposals, and historical decisions. Synthetic combat results do not establish
real-game parity or achievement of the final A20 Heart objective.

## State boundary

| State | Examples | Allowed use |
|---|---|---|
| Public | hand, HP, energy, visible piles, relics, potions, visible intent | fair observations and policies |
| Hidden | draw order, RNG, private AI state, future rewards | simulator internals and explicitly privileged research |
| Debug | snapshots, trace metadata, canonical diffs | verification only |

A fair policy receives only public state and public history. Internal IDs,
private counters, hidden order, RNG state, and future outcomes may not leak
through observations, choices, ordering, errors, or timing.

## Strategy

1. Establish honest seed-plus-action parity for the supported Ironclad surface.
2. Expand fidelity through unchanged traces and first-divergence repair.
3. Keep collection separate from verification.
4. Build agents only on explicit fair observation/action contracts.
5. Evaluate the final system on a declared A20 Heart seed and compute budget.

Real-game observations are expected output. They never mutate, anchor, or select
simulator state during replay.
