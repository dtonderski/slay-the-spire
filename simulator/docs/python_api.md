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

## Explicit synthetic combat construction

`State.from_synthetic_spec(spec_json)` creates a **new A0 combat-only state**;
it does not modify an existing run or import observed state. The strict JSON
object contains `seed` (unsigned 64-bit integer), `floor`, `kind`
(`normal`/`elite`/`boss`), `encounter`, `deck` (objects with public base `key` and
`upgrades`), `relics` (public keys), `potions` (public keys or null, in slot order),
`hp`, `max_hp`, and `gold`. Optional `counters` sets `incense_burner`, `pen_nib`,
`ink_bottle`, `happy_flower`, `sundial`, `nunchaku`, and `lizard_tail_used`.
Card objects may additionally set `ritual_dagger_damage_bonus` for Ritual Dagger.
Unknown fields, invalid counters, incompatible loadouts, and invalid floors fail.

Cards/relics are already owned: pickup effects are not replayed. Energy bonuses
are derived from owned relics; each bottle targets the first matching card.
Persistent combat counters default to zero, Lizard Tail to unused, Neow's Lament
to expired, and permanent card growth to zero. Normal combat initialization then
runs once. HP may therefore change through Blood Vial or Pantograph before the
first decision. Initial choices can include Gambling Chip or Toolbox selections.

This API is for synthetic research, not replay repair, natural-run reconstruction,
or resuming a full map. Constructor seeds and manifests are experiment metadata,
not policy observations. `rl/synthetic_roots.py` combines the floor/loadout
samplers with this constructor; see `rl/tools/README.md` for priors and limitations.

## Numeric combat batches

`State.player_hp()` returns the public context HP (`observation().context.player_hp`)
without projecting the screen or drawing RNG. Combat-start healing is included;
this is not the pre-entry loadout HP.

`State.numeric_decisions(states)` and `State.numeric_steps(states, indices, revisions)`
expose an alternative transport for combat training. The typed API above is unchanged.
Both call the same fair environment; no gameplay or replay rules are changed.
Steps are sequential and individually checked, **not atomic across the batch**:
if a later action fails, earlier accepted steps remain accepted, as in a Python loop.
Each index addresses the current public legal-action list, not an internal action id.
The paired revision must be the revision exported with that list; a mismatch is rejected
before the index is applied.

The version-3 payload is `(version, symbols, tables, model_rows)`:

- `symbols`: public strings for header/screen fields that are still batch-local.
  Content-key columns are vocabulary v1 catalog ids, not symbol positions or instance ids.
  `-1` means an absent optional category in the remaining symbol columns.
- `tables[name] = (width, bytes)`: row-major native-endian signed int64 columns.
  Bytes are immutable, independently owned, and remain valid after stepping/cloning.
  Empty tables may be omitted. Normalization and embedding vocabularies belong to RL.
- `model_rows`: input-state indices with a waiting-for-player combat, in input order.
  An observation owner below indexes this compact list, not all input states.

Columns (zero-based row references are transport offsets, never instance IDs):

| Table | Columns |
|---|---|
| `header` | kind code, run phase code, combat phase code or -1, HP, max HP; one row per input state |
| `player` | HP, max HP, block, energy, max energy, gold; one row per model observation |
| `player_powers` | observation owner, power key code, amount |
| `hand`, `draw`, `discard`, `exhaust`, `selection_cards` | owner, card key code, cost, upgrade level, cost-modified, cost-resets, bottled, temporary; five dynamic values followed by their five presence bits |
| `stasis` | same card columns, but owner is the global enemy row |
| `enemies` | owner, key code, HP, max HP, block, alive, slime-size code, intent key code, damage, hits, damage-present, hits-present, escaped, minion, defensive-mode, stolen gold, Stasis-present, targetable |
| `enemy_powers` | global enemy row, power key code, amount |
| `relics` | owner, key code |
| `relic_counters` | global relic row, counter key code, signed value |
| `potions` | owner, potion key code or -1, visible slot |
| `selection` | kind code or -1; one row per model observation |
| `selection_options`, `selected_slots` | owner, visible option slot |
| `action_rows` | batch-state owner, legal-list index, fixed public kind code, hand slot, potion slot, option slot, target slot, card slot, node slot, reward slot, shop slot, revision. Absent slots are -1. Kind codes index `ACTION_KINDS`, not this batch's symbol table. Order within an owner is public legal order. |

Card dynamic order is Rampage, Ritual Dagger, Windmill Strike, Steam Barrier,
and underlying combat cost. Absent optional integers have zero payload and a
false presence bit. Intent keys distinguish hidden/none from visible categories.
Groups retain the order of the public projection, including canonical unordered
piles, dead enemy entries, and actual empty potion slots.

This is a versioned **subset** of public observations, covering the existing combat
policy's inputs plus rollout outcomes. It is not a lossless encoding of every fair
field: e.g. permanent deck, known draw positions, orbs, and unused public counters
remain available through the full typed API. Noncombat rows carry outcome metadata
and actions but no combat feature rows. The exporter accepts only native fair
projections, not external mappings or observed game state.

## Synthetic scenarios

For explicitly synthetic experiments only, `State.new_synthetic(seed, ascension=0,
hp=10000, final_act=False)` initializes both HP and maximum HP. Set `final_act=True`
to enable the existing pre-run Act 4 profile; this selects a burning elite using
the simulator's named map RNG. It does not grant any keys. On a combat state,
`state.synthetic_combat_root(hp=100)` returns an independent clone with HP and
maximum HP replaced and an advanced decision revision; the source is unchanged.
HP must be positive. HP normalization does not consume RNG or rerun combat-start
effects. They are scenario construction tools, never trace hydration, replay
repair, or policy inputs. Ordinary `State.new` and verifier replay are unaffected.

Run-observation schema **5** adds the public `MapNode.burning_elite` marker and
allows `TreasureScreen.chest_size == "boss"`. Combat schema remains 4; numeric
combat transport remains 1. Victory screens use kind `complete` but may still
have a legal Proceed (notably before Act 4); run collectors should check actions
rather than assuming every positive-HP `complete` screen is final.

## Typed observation discriminants

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
