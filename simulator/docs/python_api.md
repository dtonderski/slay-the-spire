# Python fair environment API

The `sts_sim` package is a thin binding over the state-owning Rust `sts_env`
environment. Fair observations are concrete immutable Python types projected
from the native fair records; there is no public `Record`/`getattr` observation
surface.

```python
from sts_sim import State

state = State.new("HUMAN1", ascension=0)
while decision := state.decision():
    observation = decision.observation
    if observation.kind == "combat":
        energy = observation.screen.player.energy
        _ = energy
    if not decision.actions:
        break
    decision = state.step(decision.actions[0])
```

`State` exposes `new`, `clone`, `revision`, `decision`, `observation`,
`legal_actions`, and `step`. `Decision` atomically carries a schema version,
revision, fair observation, and an immutable tuple of decision-local actions.
Actions expose only stable kinds and visible slots and are rejected when stale.

`observation.kind` is a closed discriminant. After `observation.kind == "combat"`,
type checkers narrow `observation.screen` to the combat screen, including nested
unions such as monster intents, orbs, and rest options. The same types are also
importable from domain modules when that is clearer than the package facade:

```python
from sts_sim.observations.combat import CombatObservation, CombatScreen
from sts_sim.observations.common import Relic, RunContext
```

Unknown mapping fields are rejected when projecting native records, so Python types fail closed if the
fair schema grows. Combat piles expose a canonical `cards` multiset and
`known_positions` (position `0` is the next draw). They do not expose `count` or
`known_order`.

Owned relics live only on `observation.context.relics`. Each `Relic` has a
decision-local `slot`, a `RelicKey`, and public `state` counters. Persistent
counters such as Ink Bottle or Girya remain visible on every screen; combat-only
counters are omitted outside combat rather than fabricated as zero. Combat
screens do not repeat owned relics, owned potions, or run metadata already on
`observation.context`. Shop and reward relic/potion offers stay on those screens
because they are not owned.

Finite content identities are generated `StrEnum` members: `RelicKey`,
`PotionKey`, `CardKey`, `MonsterKey`, `PowerKey`, `EventKey`, and `CounterKey`.
Decoder output uses those enum instances, and unknown keys are rejected. Empty
potion slots are `None`, not an empty string or a sentinel member. Enum values
are the exact fair serialized strings, so `card.content_key == "Strike_R"` and
`card.content_key is CardKey.STRIKE_R` both hold. Do not use enum ordinals as an
ML vocabulary; build an explicit versioned mapping from these identity values to
tensor indices.

Event choice labels stay ordinary strings because they are display text, not a
closed content catalog.

Regenerate the checked-in enums from the repository root after catalog changes:

```bash
python simulator/python/tools/generate_content_ids.py
```

The generator exports catalogs from authoritative Rust definitions via
`cargo run -p sts_env --bin export_fair_catalog` and writes
`simulator/python/sts_sim/content_ids.py`. `--check` fails if that file is stale.

The policy package deliberately has no full-state view, raw simulator IDs,
state JSON serialization, or JSON restoration. Privileged replay and snapshots
belong to verifier/debug tooling, not this API.

Run the interactive example with:

```bash
cd simulator/python
uv run python examples/showcase.py
```
