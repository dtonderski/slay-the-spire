# Python fair environment API

The `sts_sim` package is a thin binding over the state-owning Rust `sts_env`
environment:

```python
from sts_sim import State

state = State.new("HUMAN1", ascension=0)
while decision := state.decision():
    if not decision.actions:
        break
    decision = state.step(decision.actions[0])
```

`State` exposes `new`, `clone`, `revision`, `decision`, `observation`,
`legal_actions`, and `step`. `Decision` atomically carries a schema version,
revision, fair observation, and complete decision-local actions. Actions expose
only stable kinds and visible slots and are rejected when stale.

The policy package deliberately has no full-state view, raw simulator IDs,
state JSON serialization, or JSON restoration. Privileged replay and snapshots
belong to verifier/debug tooling, not this API.

Run the interactive example with:

```bash
cd simulator/python
uv run python examples/showcase.py
```
