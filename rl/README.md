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
