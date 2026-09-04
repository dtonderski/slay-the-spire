# Project Overview

## Objective

Build the strongest fair Ironclad A20 Heart player, backed by a deterministic
simulator whose fidelity is established with immutable real-game traces.

The current repository is deliberately narrower: `sts_core` implements the
simulator, `sts_verify` replays CommunicationMod traces, and `py_sts` exposes a
small Python interface. Search and learned agents are future consumers, not
simulator responsibilities.

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
