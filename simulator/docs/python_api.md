# Python fair environment API

The `sts_sim` package is a thin binding over the state-owning Rust `sts_env`
environment. Fair observations are concrete immutable Python types projected
from the native fair records; there is no public `Record`/`getattr` observation
surface. See the [fair API contract](fair_api.md) for visibility, choice identity,
and the distinction between cloning actual state and fair belief sampling.

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
A dismissable completed-room map offers `return_to_room`, which reopens its
existing room screen without entering a room or reapplying effects. The numeric
kind vocabulary appends this kind without renumbering existing kinds.

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
Each index addresses the current public legal-action list, not an internal action id.
The indexed step uses one `projected_choices` scan of the current state and the existing
successor projection. It does not add a second observation contract.
The paired revision must be the revision exported with that list; a mismatch is rejected
before the index is applied.

`numeric_steps` releases the GIL and, for larger batches, steps and exports contiguous
chunks of states on worker threads, each with at least 64 states. States are independent,
and chunk tables are merged in input order, so the payload is identical to a serial step
for any thread count. Steps are individually checked and **not atomic across the batch**:
every state is attempted, accepted steps stay accepted even if another state fails, and the
first error in input order is raised. Callers must discard the whole batch on error. A state
passed more than once, or borrowed elsewhere (e.g. by another Python thread), is rejected
before any state is stepped. The worker cap defaults to half the logical CPUs; set
`STS_NUMERIC_THREADS` to a positive integer to override it (read once per process).

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

## Ordinary Heart-enabled runs

`State.new(seed, ascension=0, final_act=True)` starts a natural-HP Ironclad run
with the existing Heart-unlocked pre-run profile enabled. At A0, initial HP/max
HP remains 80 and the ordinary starter deck/relics are unchanged. It grants no
keys: the player must collect them through the existing run actions. The burning
elite is selected once using the named map RNG before the first policy decision.
`context.final_act_available` reports the capability; `context.keys` reports
collected keys. `final_act` is keyword-only and defaults to `False`, preserving
existing callers. There is no mid-run setter on the policy API.

Rust callers use `FairEnvironment::new_ironclad_with_final_act(seed, ascension,
true)`; the existing two-argument `new_ironclad` keeps its behavior. This exposes
existing initial profile rules, not a new gameplay/parity claim or an ascension
fidelity guarantee. The present training target is A0.

## Synthetic scenarios

For explicitly synthetic experiments only, `State.new_synthetic(seed, ascension=0,
hp=10000, final_act=False)` initializes both HP and maximum HP. Set `final_act=True`
to enable the existing pre-run Act 4 profile; this selects a burning elite using
the simulator's named map RNG. It does not grant any keys. On a combat state,
`state.synthetic_combat_root(hp=100)` returns an independent clone with HP and
maximum HP replaced and an advanced decision revision; the source is unchanged.
HP must be positive. HP normalization does not consume RNG or rerun combat-start
effects. They are scenario construction tools, never trace hydration, replay
repair, or policy inputs. Default `State.new` and verifier replay are unaffected.

Run-observation schema **8** completes current visible offer metadata:

- `RewardScreen.relic_offers` contains immutable `RelicOffer(slot, content_key)`
  entries in exactly the current `take_relic_reward_at.reward_slot` order,
  including all published Calling Bell / Matryoshka rewards. The legacy primary
  `relic_offer` is retained. These are offered relics, not private relic pools.
- `RewardScreen.sapphire_key_relic_slot` identifies the offered relic surrendered
  for the Sapphire Key (`None` when unavailable); `emerald_key_offer` reports
  an unclaimed Emerald Key reward. Collected indicators stay on `context.keys`.
- `ShopScreen.cards` uses `ShopCardOffer`, a `ShopOffer[CardKey]` subtype that adds
  the full public `card` projection: visible cost, upgrade level and dynamic
  values. Its legacy `content_key` alias is retained and checked for consistency.
  Closed merchant stock remains hidden; unopened queued card rewards still
  expose only visible reward counts, never their pre-generated card identities.

These are observation changes only. Numeric combat transport and gameplay rules
are unchanged. Public visibility is backed by CommunicationMod's current reward
items/Sapphire `link` and shop-card conversion. No unseen future offers are added.

Schema 7 added the public `context.outcome` (`RunOutcome`):

- `ongoing`: a continuing run, including final-boss victory UI / Spire Heart
  dialogue with legal continuation actions. A `complete` screen is not enough.
- `death`: settled combat death, visible before its optional UI Proceed, or
  recorded noncombat/continued death.
- `act3_clear`: the Spire Heart terminal transition without entry into Act 4.
- `heart_clear`: the settled TrueVictory transition after defeating the Heart.
- `unknown_complete`: a legacy snapshot/synthetic Complete boundary without
  recorded terminal provenance. It is **not** a win or a loss target.

Core accepted death/victory transitions record the terminal reason, and snapshots
preserve it. Observation only projects that record (or visible Lost combat); it
never guesses victory from positive HP/act number or repairs old snapshots.
Contradictory provenance fails closed. Time limits and simulator exceptions are
collector events, not game outcomes: bootstrap legitimate nonterminal cutoffs,
and quarantine simulator failures instead of assigning them death rewards.

Schema 6 added `context.act_boss` (only the current act's
visible boss encounter), `context.final_act_available` (effective key-enabled
profile capability), and immutable `context.keys` (`ruby`, `emerald`, `sapphire`
collected indicators). These are public map/profile/key facts, not future bosses,
seeds, or RNG. Python uses the closed `BossEncounter` literal and `RunKeys` type.
The City boss is cached at seeded initialization using the existing encounter
lookup, so observation never draws RNG. Legacy snapshots/non-seeded City fixtures
without that cache expose `act_boss=None`; no observation-time reconstruction or
state repair occurs. Boss/key visibility follows CommunicationMod's public
`act_boss`/`keys` output (`GameStateConverter.getGameState`).

Schema 5 introduced the public `MapNode.burning_elite` marker and
`TreasureScreen.chest_size == "boss"`. Combat schema remains 4; numeric
combat transport is unchanged by these run-observation additions. Victory screens use kind `complete` but may still
have a legal Proceed (notably before Act 4); run collectors must use the explicit
outcome rather than assume every positive-HP `complete` screen is final. Numeric
combat transport is unchanged and is not a lossless macro/run observation API.

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
