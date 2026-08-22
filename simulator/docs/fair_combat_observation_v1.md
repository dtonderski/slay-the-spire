# Fair Combat Observation V1

Status: frozen historical V1 contract; current producers emit V2.
Last updated: 2026-08-22.

V2 is an additive revision documented in `fair_combat_api_design.md`: explicit
orb slots, Windmill Strike retained damage, and visible Poison/Lock-On powers.
The Python reader accepts stored V1 payloads with those additions absent.

`fair_combat_observation(&RunState)` is the sole V1 projection entry point. It
borrows the active run and combat, consumes no RNG, performs no transition, and
returns symbolic serde data. Projection starts from `RunState`, rather than a
bare `CombatState`, because potion slots and public run context live at the run
boundary.

The only public errors are `no_active_combat`, `invalid_authoritative_state`,
and `unknown_public_content`. They never contain an internal ID or hidden-state
detail. A non-combat run returns `no_active_combat`; contradictory ownership
between the run phase and its optional combat state returns
`invalid_authoritative_state`.

## Exact top-level schema

```text
FairCombatObservation
  schema_version = 1
  context { ascension, act, floor, gold }
  phase
  player { hp, max_hp, block, energy, max_energy, powers[] }
  hand[] { slot, card }
  draw_pile { count, cards[], known_order[] }
  discard_pile { count, cards[], known_order=[] }
  exhaust_pile { count, cards[], known_order=[] }
  monsters[]
  relics[]
  potion_slots[]
  selection?
  public_counters[]
```

`gold` includes gold already earned by combat effects but not yet transferred
from combat state to run state. The combat phase is one of waiting for player,
monster turn, won, or lost.

V1 does not expose a combat-turn number. Current authoritative state does not
track it independently: `player_turns_started` is a relic-mechanics counter and
is not advanced in combats without a relevant start-of-turn relic. A general
turn number belongs with the later deterministic public-history slice, not an
inference from a conditionally updated internal counter.

## Cards, slots, and piles

A public card contains:

- the stable `CardDefinition.key`, never `ContentId` or `CardId`;
- effective visible cost, whether it is modified, and whether it resets next
  turn;
- instance-local upgrade level not already represented by the content key;
- visible bottled and combat-only/generated status;
- visible Rampage and Ritual Dagger damage bonuses.

Hand records carry an explicit decision-local slot. The slot is the public hand
position and does not contain or derive from a card instance ID.

Draw, discard, and exhaust contents are serialized in canonical public-card
order, so internal storage order cannot leak. `known_order` is top-to-bottom and
contains the complete draw order only while Frozen Eye is owned. V1 has no
public-history tracker, so it does not yet represent prefixes learned from
earlier public placements. Discard and exhaust order is never projected merely
because core stores each pile as a `Vec`.

## Player and monster powers

Powers are sorted `{ key, amount }` records. Zero-valued powers are absent.
V1 allowlists all currently modeled visible player powers plus visible derived
statuses: temporary Strength/Dexterity loss, temporary Thorns, Rage, No Block,
No Draw, Double Tap, Duplication, and Bomb timers. Displayed Strength and Thorns
include their active temporary amounts; the corresponding loss/temporary power
remains present so turn-end behavior is learnable.

The monster allowlist contains Vulnerable, Weak, Strength, Artifact, Flight,
Intangible, Plated Armor, Painful Stabs, Explosive, Ritual, Spikes, Curl Up,
Anger, Metallicize, Malleable, Spore Cloud, Strength Up, Slow, and Guardian's
displayed Mode Shift amount. Temporary monster Strength restoration is exposed
separately from current Strength. Minion and defensive mode are separate
public flags.

Private transition and AI fields are absent. In particular, V1 omits move
history, rolled attack damage, Flight's queue-settling flag, Book of Stabbing's
private progression, Spiker's private buff count, Malleable's base value,
private AI countdowns/move modes, queues, pending callbacks, and RNG.

## Monsters and intent

Monster records carry visible position slot, stable monster definition name,
explicit slime size where applicable, HP, max HP, block, public powers, intent,
stolen gold, a public Stasis card when present, and
alive/escaped/minion/targetable/defensive-mode flags. Stasis card instance
identity and `MonsterId` are absent.

Visible intent uses the target game's UI categories and exposes displayed
per-hit damage and hit count only for attacks. Damage includes current monster
Strength/Weak and player Vulnerable, matching the displayed intent number.
Buff, block, and debuff magnitudes are not exposed when the UI category does
not display them.

Runic Dome produces exactly `hidden`; the actual category, damage, and hit
count are absent. Dead monsters produce `none`. Pending or source-specific
unknown intents produce a visible `unknown` category without hidden detail.

## Relics, potions, and counters

Relic order and potion slots follow their visible UI positions. Relics use the
stable trace name already maintained by core. Potions use an explicit
lowercase public-key mapping and empty slots contain no identity.

Relic state is an explicit allowlist. V1 includes progress or availability for
Lizard Tail, Ink Bottle, Ornamental Fan, Nunchaku, Pen Nib, Shuriken, Kunai,
Letter Opener, Happy Flower, Sundial, Incense Burner, Centennial Puzzle,
Akabeko, Pocketwatch, Art of War, Orange Pellets, Necronomicon, Self-Forming
Clay, Red Skull, Velvet Choker, Horn Cleat, Captain's Wheel, Stone Calendar,
Omamori, Maw Bank, Ancient Tea Set, Girya, Matryoshka, Tiny Chest, Wing Boots,
and Neow's Lament. These values are visible in the UI or derivable from public
action history.

The generic public counters are cards and attacks played this turn. Private
Fairy/action-queue state and unrelated run RNG/reward counters are absent.

## Visible selections

The active combat selection, if any, contains a public selection kind, visible
selectable card options, and selected public slots. Candidate filtering reuses
core's authoritative selection-index mapping; the observation does not invent
a second eligibility rule. Hand and offered-card options preserve their visible
order. Options originating in draw, discard, or exhaust are canonically ordered
before receiving decision-local slots, preventing an internal pile index from
becoming a public identifier. When duplicate options have the same public card
value, selected slots are matched by that value and multiplicity, so hidden
source indices cannot break ties. Source card IDs, pending actions, and queued
decisions are absent.

## Non-interference evidence

Unit coverage asserts byte-identical serde output after:

- hidden draw/discard/exhaust permutations without Frozen Eye;
- all combat RNG stream changes;
- card and monster instance-ID renumbering;
- private monster AI, power-progress, relic, and pending-queue changes;
- hidden intent changes under Runic Dome; and
- repeated projection of the same state.

Separate boundary tests assert Frozen Eye reveals top-to-bottom draw order,
visible intent changes and damage modifiers are represented, public
cards/relics/potions/counters are present, selections do not leak source pile
order, projection does not mutate state, and public errors contain no internal
identity.

## Fairness verification contract

Fairness is tested as observational non-interference. For states that differ
only in unavailable information, serialized observations and coarse public
errors must be equal. The test matrix mutates pile order, instance IDs, RNG,
private monster and relic counters, AI history, queues, limbo, and hidden
intent. The complementary positive tests require changes to public HP, hand
order, pile membership, visible intent, and gold to change the observation.

The visibility ledger for V1 is:

| Surface | Public source | Ordering rule |
| --- | --- | --- |
| Run context and player | Current run/combat UI state | Scalar values; powers sorted by key |
| Hand | Visible hand positions | Preserve hand order; assign decision-local slots |
| Draw pile | Public pile contents; Frozen Eye for order | Canonical multiset, plus top-to-bottom order with Frozen Eye |
| Discard and exhaust | Public pile contents | Canonical multisets; storage order is hidden |
| Monsters and intent | Visible monster UI | Preserve monster slots; Runic Dome hides intent details |
| Relics and potions | Visible UI identities and allowlisted counters | Preserve visible slots; counters sorted by key |
| Selections | Authoritative core eligibility mapping | Canonicalize hidden-pile options before public slots |

This verifies absence of hidden leakage, not that every projected field is
useful for learning. Any future field must be added to this ledger with a
player-visible or public-history justification and a paired hidden-state test.
