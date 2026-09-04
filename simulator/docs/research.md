# Parity Research Notes

Read this before changing RNG, action queues, save loading, or
map/reward/shop generation. Code and immutable traces are the regression record;
this file retains only source findings that are difficult to recover from code.

Target source refers to the PC `12-18-2022` desktop JAR unless stated otherwise.

## Authorities and prior art

- [CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod) is
  the primary real-game observation/control source. Its output is not complete
  hidden state.
- [sts_lightspeed](https://github.com/gamerpuppy/sts_lightspeed) is the closest
  simulator prior art for named RNG streams, save counters, Java/libGDX RNG,
  action queues, maps, rewards, shops, and content. It is a secondary oracle,
  never game authority.
- [spirecomm](https://github.com/ForgottenArbiter/spirecomm) is useful for
  CommunicationMod client/schema patterns.
- `rusted-spire`, `conquer-the-spire`, `bottled_ai`, `borg_the_spire`, and
  `gym-sts` are architectural comparisons, not parity authorities.
- RunHistoryPlus and run-history datasets support distribution checks, not
  transition parity.

The simulator keeps explicit action queues and named RNG streams. Immediate-
effect simplifications are unsuitable for exact replay.

## Seed and RNG

`SeedHelper.getLong` uppercases a seed, maps `O` to `0`, and parses base 35 with
`0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`. Captured checks:

- `VERIFY01` → `1957307888551`
- `CODEX03` → `22079335078`
- `CODEX04` → `22079335079`

Target `Random` wraps libGDX `RandomXS128`. Integer bounds are inclusive and
each public wrapper draw increments its counter. `Collections.shuffle` behavior
and whether it receives a Java `Random` seeded by `randomLong()` or the raw game
RNG are call-site-sensitive.

Relevant save counters include potion, relic, event, monster, merchant,
card-random, card, and treasure seed counts. Mid-run import requires an audited
mapping from each save field to a local stream; a base seed alone is
insufficient.

### Process-global RNG

Some gameplay calls use process-global `MathUtils.random`, whose position is not
derivable from the run seed. The audited vanilla case is The Courier's colored
replacement identity. Initial merchant stock and colorless replacement remain
seeded. Capture any non-seeded gameplay draw as a typed call-time external input;
never infer it from the observed result. See [`verification.md`](verification.md).

## Map and encounter generation

Target bytecode establishes:

- Act 1 map RNG starts from `seed + 1`.
- Maps have 15 rows, 7 columns, and 6 paths; topology is generated before room
  assignment and redundant destination edges are removed.
- Fixed rows are combat at 0, treasure at 8, and rest at 14 in normal runs.
- Act 1 discretionary chances are shop `.05`, rest `.12`, event `.22`, elite
  `.08`; room counts use `Math.round`.
- Room-list shuffle passes raw `RandomXS128` to `Collections.shuffle`, so raw
  state advances without incrementing the wrapper counter.
- Encounter lists use normalized weighted pools, first-strong exclusions, and
  no-repeat-last-two retries.
- Entering a room reinitializes monster HP, AI, shuffle, card-random, and misc
  streams from `seed + floor`.
- Combat deck shuffle uses Java `Collections.shuffle(new Random(shuffleRng.randomLong()))`.

Captured `VERIFY01` and `CODEX04` traces pin topology, room assignment, early
encounters, pile orientation, and floor-offset HP/AI behavior.

## Rewards, relics, and potions

Normal potion rewards use `40 + potionChance`, roll `potionRng.random(99)`,
change the chance by ±10, then roll rarity and retry pool identities until the
rarity matches. Lab and Woman in Blue instead draw direct pool indices without
a rarity roll.

Normal fights do not grant relics; elite rewards roll common/uncommon/rare with
target thresholds. Relic pools are Java-shuffled from `relicRng.randomLong()`
and ordinary offers pop from the front. Rejected spawn candidates are removed
and retries pop from the back. Only Exordium initializes the pools; later acts
retain depletion order.

Cursed Key selects its curse with `cardRng` and queues obtain publication rather
than mutating the deck synchronously. Event/reward obtain timing must follow the
target action/effect lifecycle rather than an observed deck snapshot.

## Source-backed interaction findings

- Summoned Gremlins consume an identity draw and an otherwise ignored opening
  AI roll before their fixed opening move.
- Writhing Mass attack-triggered rerolls are queued, consume AI RNG, and update
  move history; Mega Debuff adds Parasite but no ordinary debuffs.
- Secret Portal eligibility depends on target gameplay time, not wall-clock
  trace timestamps. Target playtime is an explicit input, not action-completion
  evidence.
- Neow obtain ordering can depend on whether an update occurs before the choice;
  any timing input must come from pre-state/action metadata, never post-state.
- Mushrooms `Stomp` followed by target-only `Fight` confirmation is one semantic
  simulator event choice; the encounter has three Fungi Beasts.
- Random card upgrades seed Java shuffle from `miscRng.randomLong()`; Neow paths
  consume a hidden misc draw before relic equip.
- Distilled Chaos constructs three PlayTop actions up front. Selected cards stay
  in limbo and are excluded from an intervening empty-deck shuffle.
- Boss identity first follows unseen-profile progression; traces requiring it
  carry explicit `boss_unlocks` input.
- Dead Adventurer searches consume encounter RNG immediately; safe rewards and
  failed-fight exposure occur in that transition.
- The Library adds rolled cards to the bottom at index zero, reversing visible
  grid order relative to RNG roll order.
- Act 4 key acquisition, burning-elite buff selection, Shield/Spear cycles, and
  Heart powers/order are pinned from target bytecode and real traces.

## Collection timing lesson

A historical SuperFastMode fork multiplied gameplay delta and caused frame-rate-
dependent skipped card retrieval. A later fork fixed action ticks but still
corrupted the dungeon playtime clock. Those cohorts remain non-authoritative.
Collection acceleration must separate visual timing from gameplay action ticks
and the raw dungeon clock.

## Research discipline

For new parity-sensitive behavior, record the target class/method or immutable
trace in the change and test. Prefer decompiled target source plus trace evidence.
Do not create a standalone design note for each fix; Git history, tests, and the
trace corpus hold routine evidence. Update this file only for a durable finding
that future implementation cannot readily recover.
