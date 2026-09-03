# Unified Typed Python Run API

The public Python package is `sts_sim`. `RunEnv` is the single state-owning
environment and `Action` is the single player-action type. The typed facade is
implemented over the private PyO3 extension `sts_sim._native`.

```python
from sts_sim import RunEnv

env = RunEnv.combat_fixture()
decision = env.decision()
action = decision.actions[0]
result = env.step(action)
```

`decision()` returns one `Decision` containing the current fair combat
observation when that projection is supported, the single legal `Action` tuple,
and the monotonic revision. `step()` accepts one of those actions, resolves its
opaque internal command in Rust, and returns the next decision.

State access is explicit:

```python
fair_observation = env.observation()
privileged_state = env.full_state()
snapshot = env.snapshot()
restored = RunEnv.from_snapshot(snapshot)
```

For controlled projection experiments, `RunEnv.from_state_json_for_debugging`
accepts a validated privileged state JSON. It is a debugging seam, not a fair
observation input.

`observation()` is visibility-safe and tagged by the active decision screen;
combat, map, event, reward, treasure, rest, shop, card-grid, and complete
screens are represented. Combat schema V2 exposes typed `FairOrbSlot`/
`FairOrb` values and `FairCardDynamicValues.windmill_retain_damage`; these types
are exported from `sts_sim`. Producers emit schema 2 with required `orb_slots`.
The Python reader rejects any other `schema_version` and rejects payloads that
omit `orb_slots`. Optional card-dynamic fields such as Windmill retained damage
may be absent when the native projector omits nulls.

`full_state()` is a detached dictionary for debugging
and omniscient research; it is not a stable persistence format or fair model
input. `snapshot()` is the versioned, validated restoration artifact.

For controlled experiments, the package exposes typed content catalogues and
explicit debug mutation helpers. These acquire content through the simulator's
normal validation and modeled acquisition effects, and advance the decision
revision so previously returned actions become stale:

```python
from sts_sim import Card, Potion, Relic

env.add_card(Card.BASH)
env.add_relic(Relic.INK_BOTTLE)
env.add_potion(Potion.FIRE)
```

`Card`, `Relic`, and `Potion` are generated Python `StrEnum` catalogues from
the native content definitions; arbitrary strings are not accepted by these
helpers.

The old split environment and exact-action bindings remain in private native or
compatibility implementation modules during migration, but are not exported by
the package root. Every legal non-combat action now has a public decision-local
slot descriptor while its authoritative command remains private. The target contract is documented in
`design_unified_python_run_environment.md`.

Every Python source file and test is checked by `ty` and Ruff's annotation
rules. The native extension has a checked `_native.pyi` contract, and the
package includes `py.typed` for downstream type checkers.

Run the Python checks from the Python project directory:

```bash
cd python
uv sync --extra rl --reinstall-package sts-sim
uv run ty check
uv run ruff check
uv run pytest -q tests
```

The explicit reinstall step builds the private PyO3 extension before pytest
imports `sts_sim._native`, and also makes the clean-checkout CI path explicit.
