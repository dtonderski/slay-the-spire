# First contact with the simulator

This is an API walkthrough, deliberately **not** an RL framework. You own the
representations, rewards, vectorization, learning, and search.

From the repository root:

```bash
uv sync --project simulator/python --reinstall-package sts-sim
uv run --project simulator/python python rl/examples/fair_simulator.py
uv run --project simulator/python python rl/examples/fair_simulator.py --inspect --max-decisions 3
```

Start at `choose_action` in [`fair_simulator.py`](fair_simulator.py). The random
policy is only plumbing validation, not a sensible playing baseline. Increase
`--max-decisions` to explore further; random play can waste actions on reversible
selections. `--ascension 20` changes difficulty, not fidelity guarantees.

## The contract

1. `State.new` creates the environment. The driver knows the game seed; the
   policy does not.
2. `state.decision()` returns an atomic observation and its legal candidates.
3. The policy sees `observation.context` (HP, gold, deck, etc.) and the
   screen-specific `observation.screen`. Use `observation.kind` to distinguish
   combat, map, event, reward, and other decisions; after
   `observation.kind == "combat"`, type checkers narrow `observation.screen` to
   the combat screen. `--inspect` prints public values, sample combat fields,
   and candidate slots.
4. Choose from **this** candidate list and pass the original `Action` to `step`.
   Slots and candidate indices are decision-local, not universal action IDs.
   Candidate count varies; choosing a card can open another selection decision.
5. `step` returns the next decision, not a Gym `(obs, reward, terminated, …)`
   tuple. Reward design and outcome classification are intentionally left to you.

An action is not necessarily a combat turn. Do not treat a demo cutoff as death,
a missing action list as victory, or `complete` alone as proof of a Heart kill.
Failures should be reported separately from gameplay outcomes.

The Python observation types are in
[`observations.py`](../../simulator/python/sts_sim/observations.py). Action
bindings remain in [`_native.pyi`](../../simulator/python/sts_sim/_native.pyi).
Public field schemas are in
[`run_observation.rs`](../../simulator/crates/sts_env/src/run_observation.rs)
and [`combat_observation.rs`](../../simulator/crates/sts_env/src/combat_observation.rs).
Read those for feature definitions rather than reaching into core state.

## What “fair” means here

**Allowed:** public observations, current legal candidate kinds/visible slots,
your own randomness, and a history recorded from what the policy actually saw
and chose. Seeing the consequences of your *executed* action is normal training.
Offline learning from collected trajectories is also normal.

**Not policy features:** the environment seed, revision counters, hidden pile
order, private AI state, RNG state, snapshots, unrevealed rewards, or verifier
metadata. Keep timing and exceptions out of the policy too. The demo passes
native actions for convenience; their `revision` exists for stale-action
rejection, not tensorization. This separation is a convention, not a sandbox.

**The search trap:** `State.clone()` preserves the actual hidden state. Trying
several actions on clones of the live environment and selecting the best result
is privileged lookahead, even if every resulting observation is public. It can
reveal the actual next draw, enemy move, or reward before committing an action.
Do not use it for a claimed fair planner.

Fair search needs hypotheses drawn from a declared belief model conditioned only
on public information/history, independently of the environment's true hidden
state. This Python API does not currently provide a public-belief constructor.
That is a separate design discussion, not something this demo works around.

Public exposure is also not proof of simulator fidelity. Gameplay parity needs
real-game traces; see the [project objective](../../PROJECT_OVERVIEW.md) and
[boundary audit](../docs/fair_observation_hidden_state_audit.md), including known
public information that is not yet exposed.

## Suggested first exercise

Run `--inspect`, find a combat decision, and explain what one `play_hand_slot`
candidate means in terms of the displayed hand and target slots. Then replace
only `choose_action` with a tiny public-information heuristic of your choosing.
Before tensorizing anything, decide which information should survive hand
reordering and which genuinely depends on position. No training stack needed.
