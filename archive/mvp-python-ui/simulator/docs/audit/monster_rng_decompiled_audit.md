# Monster RNG Decompiled Audit

Date: 2026-07-02

Update, 2026-07-02: the local combat field `CombatState.monster_rng` is now
documented in code as target `AbstractDungeon.aiRng`, and combat entry no
longer advances `monsterHpRng` by monster count after source-backed spawn
helpers have already consumed constructor/private HP draws. Source-locked
initial intents no longer consume an extra combat AI roll; unlocked monsters
still consume the normal target-style `rollMove()` integer and may ignore the
value in deterministic `getMove` implementations. The broader per-monster and
group-composition gaps below remain open unless explicitly noted elsewhere.

Update 2, 2026-07-02: combat-entry and turn-prep AI now use source-style
helpers for the previously missing "already close" batch: Red Slaver, Snecko,
Book of Stabbing, Bronze Orb, and Orb Walker. Book of Stabbing and Orb Walker
gained compact target helpers for their decompiled `getMove(int num)` tables.
Focused tests pin that combat entry consumes one `aiRng.random(99)` and routes
through those helpers. Remaining work still includes exact Book private
`stabCount` persistence beyond the represented move-history derivation, Bronze
Orb Stasis card-selection proof, broader recursive-reroll monsters, and
`MonsterHelper` composition parity.

Update 3, 2026-07-02: Bronze Automaton orb summons now pass the live combat
`aiRng` and `monsterHpRng` streams into the spawned Bronze Orbs. Each orb
consumes the source constructor HP roll (`52..58`), then the real `setHp` roll
(`52..58`, or `54..60` at A9+), and consumes one target-style opening AI roll
for its initial Stasis/Beam/Support intent.

Update 4, 2026-07-02: Reptomancer encounter construction now mirrors target
`MonsterHelper` composition (`Dagger`, `Reptomancer`, `Dagger`) and HP stream
order: Dagger HP, Reptomancer constructor HP, Reptomancer `setHp`, Dagger HP.
Reptomancer's first move now consumes the normal opening AI roll but sets Spawn
Dagger, and Spawn Dagger creates Snake Daggers with source HP/opening AI rolls
instead of falling through the Gremlin Leader summon path.

Update 5, 2026-07-02: Taskmaster's City spawn path is now pinned as consuming
the source constructor HP roll plus `setHp` roll, and combat entry consumes one
normal `aiRng.random(99)` roll while ignoring its value for fixed Scouring Whip.
Move history now records Taskmaster's source move byte `2`.

Update 6, 2026-07-02: Collector-spawned Torch Heads now use the source HP
stream shape: constructor roll is always `38..40`, then `setHp` uses `38..40`
or `40..45` at A9+. Spawn `init()` still consumes one ignored `aiRng.random(99)`
roll for the fixed attack, and focused coverage pins HP/AI counters plus move
history byte `1`.

Update 7, 2026-07-02: Transient now follows the source direct-set-move stream
shape. Combat entry is source-locked with no opening AI roll, A4+ starts at 40
damage, and post-turn intent preparation sets the next escalating attack without
consuming `aiRng` because the decompiled class does not queue `RollMoveAction`.

Update 8, 2026-07-02: Donu/Deca boss construction now has a source-shaped pair
helper: Deca then Donu, fixed 250/265 HP by A9, 2/3 Artifact by A19, one ignored
opening `aiRng.random(99)` per monster through normal `RollMoveAction`, Deca
opening Beam, Donu opening Circle/strength, and source move bytes `0`/`2`.
Beyond Act 3 boss selection now shuffles the target boss list and can produce
the Donu/Deca pair instead of falling back to a generic fixture. Deca's Square
execution now applies block to every living monster and adds A19 Plated Armor to
each living monster.

Update 9, 2026-07-02: Looter and Mugger post-attack move preparation now follows
the decompiled direct `SetMoveAction` shape instead of consuming a generic
`RollMoveAction` integer. Looter consumes the source 0.6 speech boolean after
the first Mug and the 0.5 Smoke/Lunge boolean after the second Mug. Mugger
consumes source attack voice `aiRng.random(2)` draws, plus the second-Mug 0.6
speech boolean and 0.5 Smoke/Big Swipe boolean.

Update 10, 2026-07-02: Repulsor now has a source-style one-roll helper for
`getMove(int num)`: attack only on `num < 20` when the previous move was not
Attack, otherwise add two Dazed to the draw pile. Combat entry and turn prep
route Repulsor through this helper instead of the representative alternating
fallback.

Update 11, 2026-07-02: Exploder now has a source-shaped ignored-roll countdown
helper. Combat entry and turn prep consume the normal `RollMoveAction` AI
integer but ignore its value, producing two attacks followed by the source
Unknown/no-op move byte `2` instead of representative parity alternation.

Update 12, 2026-07-02: Gremlin Wizard post-turn move preparation now follows
the source direct `setMove` cycle without consuming `aiRng`: charge, charge,
blast, then repeat below A17; at A17+ the Wizard keeps blasting after the first
blast. Focused coverage pins the no-roll transition and source move bytes.

Update 13, 2026-07-02: Spheric Guardian now has a source-shaped helper for its
fixed first/second moves and last-big-attack alternation. Combat turn prep
still consumes the normal ignored `RollMoveAction` integer, records source move
bytes `2/4/1/3`, and preserves the source setup of Barricade, Artifact 3, and
40 starting block.

Update 14, 2026-07-02: The Maw now uses a source-shaped one-roll move helper:
opening Roar applies Weak/Frail for 3 or 5 turns by A17, later rolls choose Nom
when `num < 50` and the previous move was not Nom, Nom hit count scales from the
source private turn counter, and post-attack moves route to Drool/Strength or
Slam with source move bytes `2/3/4/5`.

Update 15, 2026-07-02: Spire Growth now uses a source-shaped one-roll helper:
A17+ opens with Constrict if the player is not already Constricted, normal rolls
use Quick Tackle below 50 unless last two tackles block it, Constrict is applied
when missing and not repeated, and Smash/Quick Tackle fallback honors source
move bytes `1/2/3`. Local player powers now include Constricted and apply its
end-of-player-turn HP loss.

Update 16, 2026-07-02: Giant Head now uses a source-shaped one-roll helper and
countdown-derived state: A18 shortens the setup countdown, Glare/Count obey the
roll and last-two history guards, It Is Time ramps from 30/40 by A3 in +5 steps
up to the source cap, source move bytes `1/2/3` are recorded, fixed 500/520 HP
by A8 is applied, and Slow is represented on the monster at combat setup.

Update 17, 2026-07-02: Nemesis now uses a source-shaped helper for its first
move, Scythe/Burn/Tri-Attack roll table, replacement `aiRng.randomBoolean()`
draws, source move bytes `2/3/4`, fixed 185/200 HP by A8, A18 Burn count, and
post-turn Intangible application. Monster Intangible now caps incoming damage
at 1 while active.

Update 18, 2026-07-02: Snake Dagger/Dagger explode execution now follows the
source queued action shape more closely: after the second fixed move attacks
for 25, the dagger loses all current HP, clears block, dies, and does not
prepare a follow-up intent or consume another AI roll.

Update 19, 2026-07-02: Exploder execution now carries the source pre-battle
`ExplosivePower(3)` in local monster state. When the fixed third-turn
Unknown/Explode move resolves, it deals the power amount to the player, clears
the power, kills Exploder, clears block, and skips follow-up intent preparation
or post-death AI rolls.

Update 20, 2026-07-02: Spiker setup/execution now mirrors the decompiled
Thorns state more closely. Local Spiker starts with 3 Thorns, 4 by A2, plus
the source A17 +3 pre-battle bonus; each buff move increments a hidden
thorns-buff counter and applies exactly +2 Thorns, and the move helper forces
attacks once that hidden counter is greater than 5.

Update 21, 2026-07-02: Bronze Automaton now has a source-shaped, history-aware
boss cycle helper. The local cycle records source move bytes `4/1/5/2/3`,
uses Boost block 9 or 12 at A9+, uses Boost Strength 3 or 4 at A4+, fires
Hyper Beam on the source counter, and follows the A19 post-Hyper-Beam Boost
branch instead of Stun while still consuming the normal ignored
`RollMoveAction` AI integer.

Update 22, 2026-07-02: Book of Stabbing now tracks the decompiled private
`stabCount` as hidden monster state instead of deriving it only from visible
move history. The count starts at 1, increments when the next intent is
multi-stab, and at A18+ also increments when Big Stab is selected; combat
entry and turn prep both mutate that stored count during source-style
`getMove` selection. Book move-history recording now uses source bytes `1` for
multi-stab and `2` for Big Stab.

Update 23, 2026-07-02: Shelled Parasite's first-move stream now matches the
decompiled source. Below A17, after the normal `RollMoveAction` integer, the
helper consumes a source `aiRng.randomBoolean()` to choose Double Strike versus
Life Suck; at A17+ it ignores the roll value and fixes Fell without that
boolean draw.

Update 24, 2026-07-02: Gremlin Tsundere's Protect action now mirrors the
decompiled `GainBlockRandomMonsterAction`: it chooses a non-source,
non-escaping, non-dying target with `AbstractDungeon.aiRng.random(size - 1)`,
including the one-candidate case, and falls back to self when no valid target
exists. After Protect, Tsundere directly sets the next move to Protect while
another monster is alive or Bash when alone, without an extra `RollMoveAction`
AI integer.

Update 25, 2026-07-02: Sentry now uses the decompiled fixed move surface while
preserving source roll timing. Combat entry and post-turn preparation consume
the normal `RollMoveAction` `aiRng.random(99)` integer and ignore its value;
the first move is Bolt/Dazed for even group indices and Beam/attack for odd
indices, then later moves alternate from the last source move byte `3`/`4`.

Update 26, 2026-07-02: Lagavulin's sleeping wake timing now follows the
decompiled direct-transition shape. Initial sleep and the first two idle sleep
turns consume normal ignored `RollMoveAction` AI integers, but the third
natural idle wake directly sets the attack move without an extra AI roll.
Damage wake still sets the Stun/Open move and then consumes one ignored roll
before selecting the attack. Lagavulin source move bytes `5/4/3/1` are now
recorded.

Update 27, 2026-07-02: Gremlin Nob's A18 move table now follows the
decompiled history-only branch. After the source ignored `RollMoveAction` roll,
A18+ ignores the roll value, prefers Skull Bash unless either of the previous
two move slots was Skull Bash, falls back to Rush unless the last two moves
were Rush, and then forces Skull Bash. Sub-A18 keeps the `num < 33` Skull Bash
branch and Rush history guard.

Update 28, 2026-07-02: Red Slaver's first move now follows the decompiled
`firstTurn` guard. Combat entry still consumes the normal ignored
`RollMoveAction` AI integer, but empty move history always opens with Stab even
when the roll is high enough for later Entangle. Later turns keep the source
Entangle/Scrape/Stab roll and history table.

Update 29, 2026-07-02: small Spike Slime now has focused coverage for its
source-shaped fixed attack. The decompiled `SpikeSlime_S` still queues normal
`RollMoveAction` calls after attacks, so combat entry/followup preparation
consume the ignored `aiRng.random(99)` integer, but `getMove(int num)` ignores
the value and always selects Attack.

Update 30, 2026-07-02: Snecko's source table now has focused helper coverage.
The opening `firstTurn` Glare/Confusion consumes the normal `RollMoveAction`
integer but ignores its value, later `num < 40` selects Tail
attack+Vulnerable/Weak, high rolls select Bite unless the last two moves were
Bite, and combat entry plus turn prep route through this helper.

Update 31, 2026-07-02: Chosen's `getMove` table is now source-locked with
focused helper coverage. A17+ opens with Hex after the normal ignored
`RollMoveAction` integer; below A17 opens with two-hit Poke, then uses Hex
once. After Hex, Chosen uses the source `num < 50` Debilitate versus Drain
branch unless the previous move was Debilitate or Drain, then falls back to
`num < 40` Zap versus Poke.

Update 32, 2026-07-02: Snake Plant's move table and effect surface now have
focused source-backed coverage. The helper preserves the source one-roll table:
`num < 65` Chompy Chomps unless the last two moves were attacks, high rolls
Spores unless history blocks Spores, and A17+ expands that Spores guard from
last move to last or previous move. Local construction starts Malleable at 3
and Spores applies Frail 2 plus Weak 2.

Update 33, 2026-07-02: Centurion/Mystic source behavior is now tighter.
Centurion Protect executes through the same `GainBlockRandomMonsterAction`
shape as the target, consuming `aiRng.random(size - 1)` to choose a valid
non-source ally before the normal post-turn `RollMoveAction` integer. Mystic's
helper now has focused coverage for missing-HP heal thresholds, A17 heal/attack
history changes, attack+Frail, and Strength-all fallback.

Update 34, 2026-07-02: Fungi Beast's detailed source row was corrected: the
decompiled `usePreBattleAction` applies Spore Cloud 2, not Artifact. Focused
coverage now pins the `num < 60` Bite/Grow table, the A17 Grow +1 bonus, Spore
Cloud setup without Artifact, and Spore Cloud's death release only when combat
is not ending.

Update 35, 2026-07-02: small Acid Slime combat-entry AI now preserves the
source ascension split. Entry still consumes the normal `RollMoveAction`
`aiRng.random(99)` integer. Below A17, `getMove` then consumes the source
`aiRng.randomBoolean()` to choose Tackle versus Weak; at A17+ the empty-history
opening goes straight to Weak without that boolean draw. Focused coverage pins
the A16 two-draw and A17 one-draw counter difference.

Update 36, 2026-07-02: Jaw Worm's recursive/replacement draw-count concern is
now source-backed with focused coverage. The helper consumes no extra AI draw
for unguarded threshold branches, and consumes exactly one replacement
`aiRng.randomBoolean` for each guarded source branch: last Chomp on low rolls,
last two Thrashes on mid rolls, and last Bellow on high rolls. Remaining Jaw
Worm work is horde setup and broader trace-backed action-order validation.

Update 37, 2026-07-02: Louse private `monsterHpRng` interleaving is tighter in
mixed Exordium groups. `Exordium Thugs` and `Exordium Wildlife` now attach a
rolled Curl Up power when their weak slot selects a louse, with the roll delayed
until after the source candidate constructor/private HP draws. Focused coverage
pins the expected hidden Curl Up amount for both mixed helpers.

Update 38, 2026-07-02: large Acid Slime move selection now uses source move
history instead of inferring from only the previous intent. The helper preserves
the decompiled last-two guards for repeated Wound Tackle and Normal Tackle,
including the replacement boolean draws below A17 and at A17+. Entry, turn
prep, and split follow-up call sites now pass move history into the helper.

Update 39, 2026-07-02: medium/large Spike Slime collapsed-size handling is now
tighter. The move helper applies source Frail amounts for medium versus large
slimes, including A17+ large Frail 3, and generic collapsed-id adaptation uses
`max_hp` rather than current HP for large attack/count/Frail identity. Large
Spike Slime split children now use the source constructor shape where current
split HP becomes the child max HP. Focused coverage pins the A17/sub-A17
history guards and split-child max HP.

Update 40, 2026-07-02: Byrd grounded and airborne transitions now mirror the
decompiled source more closely. Grounded turn prep consumes the normal
`RollMoveAction` AI integer, ignores the roll, and fixes Headbutt. Headbutt
then directly sets Go Airborne without another AI roll; Go Airborne is encoded
as source move byte `2` and reapplies Flight instead of adding Strength. Focused
coverage pins the grounded one-draw path and the no-roll Headbutt-to-airborne
transition. Remaining Byrd risk is broader trace validation across multi-Byrd
groups and damage-triggered Stun/Flight timing.

Update 41, 2026-07-02: Gremlin Leader's recursive replacement move draws now
use the decompiled ranges instead of fixed representative rolls. When one
gremlin is alive and Rally is blocked on the low branch, the helper consumes
`aiRng.random(50, 99)` before re-entering `getMove`; when Stab is blocked on
the high branch, it consumes `aiRng.random(0, 80)`. Combat entry and turn prep
thread the live AI stream into those replacements, and focused coverage pins
both draw ranges and counters. Remaining Gremlin Leader risk is summon
slot/identity/action ordering and minion death/escape trace validation.

Update 42, 2026-07-02: Darkling half-dead turn flow now follows the source
Count/Reincarnate roll shape. A Darkling reduced to half-dead keeps the local
non-targetable marker (`alive=false`, `escaped=true`) but records Count as
source move byte `4`. Its Count turn consumes one normal ignored
`RollMoveAction` AI integer and prepares Reincarnate/source byte `5`; the
Reincarnate turn revives to half max HP, clears the half-dead marker, and then
consumes the next normal roll for the following move. Focused coverage pins
both half-dead transitions and AI counters. Remaining Darkling risk is exact
all-Darklings-dead room resolution, Regrow power visibility, and broader
multi-Darkling trace validation.

Update 43, 2026-07-02: Reptomancer's post-opening move table now follows the
decompiled source instead of the representative modulo cycle. Combat entry and
turn prep route through a source-shaped helper: empty history fixes Spawn
Dagger; later low rolls choose Snake Strike unless the last move was Snake
Strike, then consume `aiRng.random(33, 99)`; mid rolls summon only when
`canSpawn` is true and Spawn is not last-two; high rolls choose Big Bite unless
the last move was Big Bite, then consume `aiRng.random(65)`. Focused coverage
pins first move, can-spawn fallback, and both recursive replacement counters.
Remaining Reptomancer risk is exact summon slot/action ordering, death cleanup,
and the Snake Strike attack+Weak action surface.

## Scope

This document compares the current local monster implementation against the
local decompiled Slay the Spire Java sources under `tmp/decompiled-sts/`, with
special attention to RNG streams and draw consumption. It is documentation-only:
no simulator code was changed for this audit.

The local implementation inspected here is primarily:

- `simulator/crates/sts_core/src/content/monsters.rs`
- `simulator/crates/sts_core/src/content/encounters.rs`
- `simulator/crates/sts_core/src/combat/turn.rs`
- `simulator/crates/sts_core/src/run/map.rs`

The target sources inspected here are:

- `tmp/decompiled-sts/com/megacrit/cardcrawl/monsters/exordium/*.java`
- `tmp/decompiled-sts/com/megacrit/cardcrawl/monsters/city/*.java`
- `tmp/decompiled-sts/com/megacrit/cardcrawl/monsters/beyond/*.java`
- `tmp/decompiled-sts/com/megacrit/cardcrawl/monsters/ending/*.java`
- `tmp/decompiled-sts/com/megacrit/cardcrawl/helpers/MonsterHelper.java`

This is a status audit, not proof of complete real-game parity. Rows marked
partial identify the next source-backed work needed before implementation.

## Monster Inventory

The decompiled monster classes in scope are listed first so future work has a
fixed checklist. Local ids intentionally collapse some Java identities: one
local `ACID_SLIME_ID` covers small/medium/large Acid Slimes, one local
`SPIKE_SLIME_ID` covers small/medium/large Spike Slimes, and local red/green
louse ids correspond to the Java normal/defensive louse constructors through
spawn helpers rather than a direct class-name match.

| Area | Decompiled monster classes | Local coverage |
| --- | --- | --- |
| Exordium | `AcidSlime_L`, `AcidSlime_M`, `AcidSlime_S`, `ApologySlime`, `Cultist`, `FungiBeast`, `GremlinFat`, `GremlinNob`, `GremlinThief`, `GremlinTsundere`, `GremlinWarrior`, `GremlinWizard`, `Hexaghost`, `HexaghostBody`, `HexaghostOrb`, `JawWorm`, `Lagavulin`, `Looter`, `LouseDefensive`, `LouseNormal`, `Sentry`, `SlaverBlue`, `SlaverRed`, `SlimeBoss`, `SpikeSlime_L`, `SpikeSlime_M`, `SpikeSlime_S`, `TheGuardian` | All gameplay monsters except `ApologySlime`, `HexaghostBody`, and `HexaghostOrb` have local executable or collapsed coverage. Body/orb classes are visual/support classes, not independent local monsters. |
| City | `BanditBear`, `BanditLeader`, `BanditPointy`, `BookOfStabbing`, `BronzeAutomaton`, `BronzeOrb`, `Byrd`, `Centurion`, `Champ`, `Chosen`, `GremlinLeader`, `Healer`, `Mugger`, `ShelledParasite`, `SnakePlant`, `Snecko`, `SphericGuardian`, `Taskmaster`, `TheCollector`, `TorchHead` | All have local executable definitions, many with representative or partial AI. |
| Beyond | `AwakenedOne`, `Darkling`, `Deca`, `Donu`, `Exploder`, `GiantHead`, `Maw`, `Nemesis`, `OrbWalker`, `Reptomancer`, `Repulsor`, `SnakeDagger`, `Spiker`, `SpireGrowth`, `TimeEater`, `Transient`, `WrithingMass` | All have local executable definitions. `SnakeDagger` is represented locally as `DAGGER_ID`. Most Beyond AI remains representative/partial. |
| Ending | `CorruptHeart`, `SpireShield`, `SpireSpear` | All have local executable definitions, with major Act 4 mechanics still partial. |

Local content ids currently found: `CULTIST`, `JAW_WORM`, `GREMLIN_NOB`,
`RED_LOUSE`, `GREEN_LOUSE`, `SPIKE_SLIME`, `ACID_SLIME`, `LAGAVULIN`,
`SENTRY`, `HEXAGHOST`, `SLIME_BOSS`, `GUARDIAN`, `LOOTER`,
`SPHERIC_GUARDIAN`, `MUGGER`, `CHOSEN`, `SNAKE_PLANT`, `SNECKO`,
`CENTURION`, `HEALER`, `BYRD`, `SHELLED_PARASITE`, `BOOK_OF_STABBING`,
`TASKMASTER`, `GREMLIN_LEADER`, `FUNGI_BEAST`, `SLAVER_BLUE`,
`SLAVER_RED`, `GREMLIN_WARRIOR`, `GREMLIN_THIEF`, `GREMLIN_FAT`,
`GREMLIN_TSUNDERE`, `GREMLIN_WIZARD`, `BRONZE_AUTOMATON`, `BRONZE_ORB`,
`ORB_WALKER`, `DARKLING`, `THE_COLLECTOR`, `TORCH_HEAD`, `EXPLODER`,
`SPIKER`, `REPULSOR`, `TRANSIENT`, `BANDIT_BEAR`, `BANDIT_POINTY`,
`BANDIT_LEADER`, `CHAMP`, `AWAKENED_ONE`, `DAGGER`, `DECA`, `DONU`,
`GIANT_HEAD`, `NEMESIS`, `REPTOMANCER`, `SPIRE_GROWTH`, `MAW`,
`TIME_EATER`, `WRITHING_MASS`, `CORRUPT_HEART`, `SPIRE_SHIELD`, and
`SPIRE_SPEAR`. `FIXED_SIMPLE_MONSTER` is a local fixture and is not a target
game monster.

## Target RNG Streams

Target Java uses several different randomness sources around monsters. The
names matter because exact replay depends on advancing the same stream at the
same call site.

| Target source | Gameplay meaning | Local status |
| --- | --- | --- |
| `AbstractDungeon.monsterRng` | Encounter list generation and boss list shuffles. This is a run-level encounter-content stream, not the per-turn monster AI stream. | Local `RunState.monster_rng_seed/counter` mostly models encounter generation, but combat also uses a `CombatState.monster_rng` name for AI. This naming is dangerous. |
| `AbstractDungeon.aiRng` | Monster initial and subsequent move selection; recursive rerolls and random move-history fallbacks consume this stream. | Local `CombatState.monster_rng` is used as the AI stream during combat. The behavior is partly source-backed for several monsters, but the name should be read as target `aiRng` in combat. |
| `AbstractDungeon.monsterHpRng` | Monster HP rolls and some monster-private random values such as louse bite damage, louse Curl Up amount, Darkling Nip damage, Bronze Orb HP, Torch Head HP, Taskmaster HP, Orb Walker HP, Reptomancer HP, and Dagger HP. | Local `CombatState.monster_hp_rng` exists, but many local definitions still use fixed fixture HP or precomputed spawn helpers. Some HP draws are source-backed for early Act 1 paths. |
| `AbstractDungeon.miscRng` | Group composition and branch selection in `MonsterHelper`, e.g. Small Slimes, shapes, slimes, gremlin gangs, louse type choice, random monster selection for helper-generated groups. | Local spawn helpers reproduce selected Act 1/City/Beyond compositions, but broad `miscRng` parity is partial. |
| `AbstractDungeon.cardRandomRng` / local `card_random_rng` | Random card placement/selection effects caused by monster actions, such as Bronze Orb Stasis and Dazed/Burn insertion into draw pile random positions. | Local `card_random_rng` is used for Bronze Orb Stasis and random draw-pile status insertion. Coverage is partial and should be audited per action. |
| `MathUtils.random` / `MathUtils.randomBoolean` in monster classes | Mostly animation, sound, quote, and visual offset randomness. Some decompiled methods use it in non-gameplay speech/sound branches. | Not modeled, and generally should not be modeled for deterministic gameplay unless a specific call affects combat state. |

Critical naming note: when this document says local `monster_rng` in combat, it
means the simulator field currently used for target Java `aiRng`, not target
Java `monsterRng`.

## Local Combat RNG Timing

At combat entry, `run/map.rs` currently:

- creates `shuffle_rng` from `event_rng_seed + current_floor`
- creates `monster_hp_rng` from `event_rng_seed + current_floor` with counter
  initialized to `base.monsters.len()`
- creates `card_random_rng` from the run card-random stream
- creates local `monster_rng` from `monster_rng_seed + current_floor`, then
  calls `apply_initial_monster_ai_rolls`

This is only partially aligned with target Java naming. Prior research notes say
the target reinitializes `monsterHpRng`, `aiRng`, `shuffleRng`,
`cardRandomRng`, and `miscRng` from `Settings.seed + floorNum` on room entry.
The local use of `event_rng_seed + current_floor` for shuffle and HP works for
some captured paths only because several run seeds are currently aliases in
state. Future implementation should make the combat-entry stream names explicit
before changing behavior.

`apply_initial_monster_ai_rolls` currently consumes one `random_int(99)` for
most living monsters, even when the local fallback later ignores the roll. Some
target Java classes have fixed first moves and should not necessarily consume
`aiRng` at combat entry. This is a high-priority stream-consumption risk.

## Per-Monster RNG / AI Status

Legend:

- `source-backed`: local behavior has a target-style helper or known source
  evidence for the listed RNG behavior.
- `partial`: local behavior is executable but does not fully match target
  stream consumption, move history, HP/private rolls, or special mechanics.
- `representative`: local behavior is a deterministic or simplified stand-in.
- `not local`: no ordinary local gameplay definition.

| Monster | Target decompiled RNG / private state | Local status | Differences / risks |
| --- | --- | --- | --- |
| Acid Slime (S) | `getMove` consumes the normal target `RollMoveAction` integer, then below A17 uses `aiRng.randomBoolean()` to choose Tackle versus Weak. At A17+ it ignores the roll and, with empty history, opens Weak without the boolean draw. No HP roll in constructor. | source-backed/partial | Local collapsed `ACID_SLIME_ID` now preserves the small-slime entry draw split: two AI draws below A17, one ignored integer at A17+. Direct post-turn follow-up alternates Tackle/Weak without a roll, matching the decompiled `setMove` shape. Remaining risks are collapsed identity, poison/split-routed small slime validation, and broader trace-backed action ordering. |
| Acid Slime (M) | `getMove` uses `aiRng.randomBoolean` branches for move-history constraints. | partial | Local medium helper is closer than fixture behavior. Collapsed identity and HP-threshold routing are fragile. |
| Acid Slime (L) | `getMove` uses `aiRng.randomBoolean` replacement branches, including last-two Wound/Normal Tackle history guards; split spawns two medium slimes with `MathUtils` y-offset only. | source-backed/partial | Local large helper now uses move history, preserving the source last-two attack guards and replacement boolean draw counts. Entry, turn prep, and split follow-up call sites pass history. Remaining risks are exact split child action ordering/intent timing and broader trace-backed validation. |
| Apology Slime | Constructor rolls HP with `monsterHpRng.random(8, 12)`; AI uses `aiRng.randomBoolean`. | not local | Not ordinary Ironclad route content. If ever modeled, it has both HP and AI stream draws. |
| Cultist | First move is Incantation and ignores `num`, but `takeTurn` queues `RollMoveAction` after both Incantation and Attack. Constructor HP is fixed/ranged via superclass in target. `MathUtils` calls are speech/sound. | source-backed/partial | Local Ritual then attack surface and one ignored opening AI roll are source-shaped. Remaining gap is broader trace validation, not basic Cultist AI stream timing. |
| Jaw Worm | `getMove` uses passed `num` plus replacement-only `aiRng.randomBoolean(0.5625/0.357/0.416)` branches when source history guards block the selected move. | source-backed/partial | Local helper now has focused counter coverage for zero extra draws on unguarded branches and exactly one replacement boolean on the last-Chomp, last-two-Thrash, and last-Bellow guarded branches. Remaining gaps are horde pre-battle state and trace-backed action ordering. |
| Louse Normal / Red Louse | Constructor rolls bite damage from `monsterHpRng`; Curl Up power amount also uses `monsterHpRng`; move choice uses `num`/history. | source-backed/partial | Local red louse models rolled bite damage and rolled Curl Up. Ordinary `2 Louse`/`3 Louse` and mixed `Exordium Thugs`/`Exordium Wildlife` helpers now preserve source HP, bite, and delayed Curl Up interleaving for selected louses. Remaining gap is broader trace-backed action-order/state-import validation. Spawn kind is via `miscRng`. |
| Louse Defensive / Green Louse | Same as normal louse: bite damage and Curl Up amount use `monsterHpRng`; move choice uses `num`/history. | source-backed/partial | Local green louse models rolled bite, weak move, and rolled Curl Up. Focused coverage now pins mixed Exordium group Curl Up timing after candidate constructor/private HP draws. Remaining gap is broader trace-backed action-order/state-import validation. |
| Spike Slime (S) | Fixed attack in `getMove`; `takeTurn` queues `RollMoveAction` after the attack, so entry/after-attack preparation consumes one ignored AI integer. | source-backed/partial | Local small branch consumes the source-shaped AI roll and ignores its value; focused coverage pins combat entry. Remaining gap: trace-backed action-order, poison, and split-routed small slime validation. |
| Spike Slime (M) | Move table selected by `num`; no extra target stream in simple branches. | source-backed/partial | Local collapsed `SPIKE_SLIME_ID` now has focused coverage for medium versus large damage/count/Frail identity, sub-A17 last-two Frail guard, and A17+ last-Frail guard. Remaining gap is broader trace-backed action ordering and split-routed child validation. |
| Spike Slime (L) | Split uses `MathUtils` y-offset only; move table selected by `num`; large Frail is 2, or 3 at A17+. | source-backed/partial | Local large split exists, uses max HP for collapsed large identity, and split children now use current split HP as max HP like the target constructor. Remaining gap is exact child spawn action ordering/intent timing in longer traces. |
| Fungi Beast | Move choice is roll/history based with one normal `RollMoveAction` integer per turn; no extra `aiRng` boolean branches inside `getMove`. Pre-battle applies Spore Cloud 2, and A17+ Grow applies one extra Strength. | source-backed/partial | Local helper is used in combat-entry/turn preparation, with focused coverage for Bite/Grow thresholds, A17 Grow +1, Spore Cloud setup without Artifact, and death release only when combat is not ending. Remaining work is trace-backed action-order validation and broader HP/routing evidence. |
| Gremlin Nob | First Bellow ignores `num` after the normal `RollMoveAction` roll. Below A18, later turns use `num < 33` for Skull Bash plus Rush no-repeat guards. At A18+, the roll value is ignored and history alone chooses Skull Bash/Rush: no Skull Bash in the prior two move slots forces Skull Bash, two Rushes force Skull Bash, otherwise Rush. | source-backed/partial | Local helper and turn prep now preserve the ignored-roll action and A18 history-only branch. Remaining gap is trace-backed action-order validation for Anger/Vulnerable application and non-gameplay speech randomness. |
| Lagavulin | Sleeping Lagavulin uses normal ignored `RollMoveAction` rolls for initial sleep and the first two idle sleep turns. The third natural idle wake directly `SetMove`s the attack with no extra AI roll. Damage wake sets Stun/Open, then consumes one ignored roll before attack. Awake attack/debuff selection is state/history driven. | source-backed/partial | Local now preserves the source roll/no-roll split for sleep, natural wake, and damage wake, and records source bytes `5` Sleep, `4` Stun/Open, `3` attack, and `1` siphon. Remaining gap is broader action-order trace validation around Metallicize removal/music/visual actions. |
| Sentry | `takeTurn` always queues `RollMoveAction`, so combat entry and post-turn prep consume one ignored `aiRng.random(99)` integer. `getMove` first chooses Bolt/Dazed for even group indices and Beam/attack for odd indices, then alternates from the previous source move byte (`3`/`4`). | source-backed/partial | Local now consumes the source-shaped ignored AI roll, uses group index for the first move, records source bytes, and alternates from move history. Remaining gap is trace-backed action-order validation for Dazed insertion and multi-Sentry elite fights. |
| Looter | `getMove` always opens Mug; post-slash transitions use direct `SetMoveAction`, with source `aiRng.randomBoolean(0.6)` speech after the first Mug and `randomBoolean(0.5)` Smoke/Lunge after the second Mug. | source-backed/partial | Local turn prep now avoids generic post-attack roll integers and consumes the source direct-transition booleans for Mug->Mug, Mug->Smoke/Lunge, Lunge->Smoke, and Smoke->Escape. Remaining work is trace-backed validation of stolen-gold/reward room behavior and escape edge cases. |
| Blue Slaver | `getMove` uses `num >= 40` for Stab unless last two Stabs, otherwise Rake/Weak with A17 changing the Rake repeat guard from last-two to last-one. | source-backed/partial | Local helper uses the source thresholds, A2 damage, A17 Weak amount, and one AI integer per move. Remaining gap is trace-backed action-order validation and broader encounter routing evidence. |
| Red Slaver | First turn has a fixed Stab `firstTurn` guard after the normal ignored roll. Later turns use `num >= 75` for first Entangle, `num >= 55` Stab after Entangle with last-two guard, otherwise Scrape/Vulnerable with A17 changing the repeat guard from last-two to last-one. | source-backed/partial | Local combat entry now consumes one AI roll but fixes empty-history opener to Stab, then uses the source later Entangle/Scrape/Stab table with A2 damage and A17 Vulnerable amount. Remaining gap is trace-backed action-order validation for Entangle/Vulnerable and non-gameplay speech randomness. |
| Gremlin Fat | Target class mostly deterministic attack/debuff; random speech uses `MathUtils`. | partial | Local minion behavior exists. Gremlin Gang composition is target `miscRng`, not AI. Escape timing on leader death is modeled globally. |
| Gremlin Thief | Target class mostly deterministic attack/escape; random speech uses `MathUtils`. | partial | Local minion behavior exists. Gremlin Gang composition is target `miscRng`; stealing/escape reward timing needs checking. |
| Gremlin Warrior | Target class mostly deterministic attack/escape; random speech uses `MathUtils`. | partial | Local attack and anger surface exist. Composition is `miscRng`. |
| Gremlin Tsundere | Protect target selection is gameplay-random via `GainBlockRandomMonsterAction`, which uses `AbstractDungeon.aiRng.random(size - 1)` over non-source, non-escaping, non-dying candidates and falls back to self when none exist. After Protect/Bash, source directly calls `setMove` for the next move. | source-backed/partial | Local Protect now uses the combat AI stream for target selection, consumes it even for one candidate, and direct-sets the follow-up Protect/Bash without a post-turn `RollMoveAction` integer. Remaining gap is broader Gremlin Leader minion death-react/escape trace validation. |
| Gremlin Wizard | Fixed charge/ultimate cycle; random speech uses `MathUtils`, and post-turn transitions are direct `setMove` calls rather than `RollMoveAction`. | source-backed/partial | Local post-turn prep now avoids `aiRng`, follows charge/charge/blast repeat below A17, and keeps blasting at A17+. Remaining work is escape/death-react behavior and trace validation. |
| Slime Boss | Mostly deterministic boss cycle; split has visual `MathUtils`; source scan did not show `aiRng` in `getMove`. | representative/partial | Local boss phase is substantially simplified. Generic entry rolls and split behavior need full source-backed rewrite before parity. |
| Hexaghost | Deterministic cycle with dynamic Divider damage based on player HP; `MathUtils` calls are visual. | representative/partial | Local omits major boss details. Generic AI-roll consumption likely extra because moves are cycle/state based. |
| The Guardian | Deterministic mode/cycle state; no target AI stream branch found in `getMove`. | partial | Local mode behavior exists but simplified. Generic AI-roll consumption likely extra for a deterministic boss. |
| Spheric Guardian | Target starts with Barricade, Artifact 3, and 40 block; `takeTurn` queues normal `RollMoveAction`, while `getMove` ignores the roll value and follows fixed first/second move flags plus last-big-attack history. | source-backed/partial | Local setup, normal post-turn roll consumption, source move bytes, activate block, frail attack, and big/harden alternation now mirror the decompiled class. Remaining work is trace-backed validation of action ordering/audio-only randomness. |
| Chosen | A17+ opens with Hex after the normal ignored roll; below A17 opens with two-hit Poke, then Hex once. Later moves use `num < 50` Debilitate versus Drain unless the previous move was Debilitate/Drain, then `num < 40` Zap versus Poke. | source-backed/partial | Local helper is used at entry/turn prep and has focused coverage for first-turn Hex/Poke, one-time Hex, Debilitate/Drain, Zap/Poke fallback, and source move bytes. Remaining work is trace-backed action-order validation for Hex/Debilitate/Drain effects. |
| Byrd | `getMove` uses `aiRng.randomBoolean` branches with probabilities and flight state. Grounded Byrd ignores the roll and fixes Headbutt; Headbutt directly sets Go Airborne without a roll. Group y-offset uses `MathUtils`. | source-backed/partial | Local helper handles flying roll/replacement branches, grounded Headbutt, Go Airborne move byte `2`, and Flight reapplication, with focused coverage for grounded prep and the direct Headbutt transition. Remaining gap is trace-backed validation across multi-Byrd groups and damage-triggered Stun/Flight timing. |
| Shelled Parasite | `getMove` uses a lower-ascension first-move `aiRng.randomBoolean`, A17+ fixed Fell, and recursive rerolls when Fell is history-blocked. | source-backed/partial | Local helper now consumes the lower-ascension first-move boolean, fixes A17+ Fell without that boolean, and keeps the recursive replacement draw path. Remaining work is trace-backed action-order validation and broader City encounter coverage. |
| Snake Plant | `num < 65` chooses three-hit Chompy Chomps unless the last two moves were attacks; high rolls choose Spores unless Spores history blocks it, with A17+ checking the last or previous move. Pre-battle applies Malleable. | source-backed/partial | Local helper is used at entry/turn prep and has focused coverage for thresholds, A17 Spores guard, Malleable 3 setup/reset surface, and Spores applying Frail 2 plus Weak 2. Remaining gap is trace-backed action-order validation for bite VFX/damage sequencing and debuff ordering. |
| Snecko | First turn is fixed Glare/Confusion after the normal ignored roll; later moves use `num < 40` for Tail attack+debuff, otherwise Bite unless the last two moves were Bite. | source-backed/partial | Local helper is used for combat entry and turn prep, with focused coverage for the fixed opener, roll threshold, Bite history guard, and A17 Weak addition. Confusion cost randomization uses card-random stream elsewhere; remaining gap is trace-backed action-order/card-cost validation. |
| Centurion | Roll/history and ally-count dependent behavior; Protect uses `GainBlockRandomMonsterAction`, selecting a valid non-source ally with `aiRng.random(size - 1)` before the normal post-turn roll. | source-backed/partial | Local helper considers living monster count, Protect now blocks a random valid ally through the combat AI stream, and focused coverage pins Protect/Fury/Slash history. Remaining gap is broader trace-backed pair action ordering and death/escape edge cases. |
| Healer / Mystic | Roll/history and total missing-HP dependent behavior; Heal and Strength affect every living non-dying/non-escaping monster. Random idle/voice visuals use `MathUtils`. | source-backed/partial | Local helper considers total living missing HP and has focused coverage for A17 heal threshold, A17 attack history guard, Frail attack, and Strength-all fallback. Remaining gap is trace-backed action ordering/heal cap validation in the Centurion/Mystic pair. |
| Book of Stabbing | Roll/history based; sound uses `MathUtils`. | source-backed/partial | Local combat entry and turn prep now use a source-style roll/history helper backed by stored hidden `stabCount`, including the A18 Big Stab increment rule and source move bytes `1`/`2`. Remaining work is trace-backed action-order validation and broader elite-context coverage. |
| Taskmaster | Constructor uses `monsterHpRng.random(54, 60)`; move is fixed Scouring Whip. | source-backed/partial | Local City spawn consumes constructor plus `setHp` HP rolls, combat entry consumes one ignored AI roll for fixed Scouring Whip, and move history records source byte `2`. Wound count and A18 Strength are handled in execution; remaining work is trace-backed validation in event/elite contexts. |
| Gremlin Leader | Summon identity uses target `aiRng.random(0, list.size - 1)`; `getMove` has recursive `aiRng.random(50, 99)` and `aiRng.random(0, 80)` replacements when one-gremlin branches are history-blocked. | source-backed/partial | Local helper now consumes the source replacement ranges during combat entry and turn prep, with focused counter coverage. Summon paths exist, but exact summon slot/identity/action ordering and minion death/escape behavior still need trace validation. |
| Bronze Automaton | Mostly deterministic spawn/boost/hyperbeam cycle; orb spawn visual sounds use `MathUtils`. | source-backed/partial | Local orb spawn consumes Bronze Orb HP and opening AI streams in source order, and boss turn prep now consumes one ignored AI integer while following the source Spawn Orbs/Flail/Boost/Hyper Beam/post-beam cycle, including A19 Boost instead of Stun and source move bytes. Remaining work is death cleanup for live Bronze Orbs, exact action-order traces, and broader boss-fight validation. |
| Bronze Orb | Constructor HP uses `monsterHpRng.random(52, 58)`; Stasis selects a card with card-random behavior; move selection is roll/history based. | source-backed/partial | Local combat entry, turn prep, and Bronze Automaton summons now use the source-style roll/history helper; summoned orb HP consumes constructor plus `setHp` rolls. Stasis uses `card_random_rng`, but exact card rarity priority and target draw count proof remain. |
| The Collector | Torch Head summon positions use `MathUtils`; move selection is roll/history/minion-death dependent. | source-backed/partial | Local helper exists and Torch Head spawn uses source-shaped AI/HP streams. Remaining work is exact summon-slot replacement timing and broader Collector history/minion-death validation. |
| Torch Head | Constructor HP uses `monsterHpRng.random(38, 40)`; move fixed attack. | source-backed/partial | Collector-spawned Torch Heads now consume constructor HP plus source `setHp`, one ignored spawn-init AI roll, and record fixed attack move byte `1`. Remaining work is trace validation of replacement-slot ordering. |
| Bear | No gameplay RNG found in scan beyond fixed `getMove`; visual `MathUtils`. | representative/partial | Local event monster is representative. Need full event-fight source audit for move history and rewards. |
| Pointy | Fixed multi-hit attack; visual `MathUtils`. | representative/partial | Local representative coverage is likely adequate for surface, but target stream consumption should be zero after setup. |
| Romeo / Bandit Leader | Fixed/scripted moves; visual `MathUtils`. | representative/partial | Local representative coverage exists. Need exact Bandit event sequencing. |
| Champ | Boss phase/state driven; quote selection uses `MathUtils`; no target gameplay stream in scan. | representative/partial | Local Champ is representative. Generic AI-roll consumption likely extra; phase threshold and Execute cycle need source-backed work. |
| Orb Walker | Constructor HP uses `monsterHpRng.random(90, 96)`; move choice is simple history/roll based. | source-backed/partial | Local spawn HP helper exists and combat entry/turn prep now use a source-style roll/history helper. Remaining work is trace-backed validation of HP draw timing, Burn insertion ordering, and A17 strength-up behavior. |
| Darkling | Constructor rolls private Nip damage with `monsterHpRng`; `getMove` uses target `aiRng.random` recursive rerolls; half-dead Count/Reincarnate state matters. | source-backed/partial | Local helper models the recursive reroll ranges, and half-dead Count/Reincarnate now consumes the source roll sequence and records bytes `4`/`5`. Remaining gaps are exact all-Darklings-dead room resolution, Regrow power visibility, and broader multi-Darkling trace validation. |
| Exploder | Fixed attack/countdown/explode cycle; target queues `RollMoveAction` and ignores the roll value. | source-backed/partial | Local combat entry and turn prep now consume the normal AI integer while using the source two-attack countdown into Unknown/Explode move byte `2`; the Explosive(3) power now damages the player and kills Exploder without a follow-up roll. Remaining work is trace-backed action-order validation in shape fights. |
| Spiker | Roll/history based; thorns buff state matters. | source-backed/partial | Local helper now models source starting Thorns, A17 +3 bonus, +2 Thorns buff execution, hidden thorns-buff counter, and forced attacks after more than five buffs. Remaining work is trace-backed action-order validation in shape fights. |
| Repulsor | Roll/history based: attack only on `num < 20` and not after Attack; otherwise add two Dazed to the draw pile. | source-backed/partial | Local combat entry and turn prep now use a source-style one-roll helper, and execution uses draw-pile Dazed insertion. Remaining work is trace-backed validation of random draw-pile insertion ordering in multi-shape fights. |
| Transient | Deterministic escalating attack and turn-count death. | source-backed/partial | Local entry and post-turn prep now avoid AI rolls, use A4+ 40 base damage, escalate by 10 per turn, and preserve fading-after-turn behavior. Remaining work is trace-backed validation of Fading/Shifting powers and reward/death edge cases. |
| Spire Growth | Deterministic/roll based by `num`; no target `aiRng` branch found in scan beyond the normal roll action. | source-backed/partial | Local helper now mirrors A17 fixed Constrict opener, Quick Tackle threshold/history guard, Constrict reapply guard, Smash fallback, source move bytes, fixed A7 HP, and Constricted end-turn HP loss. Remaining work is trace-backed source-owner removal/action-order validation. |
| Maw | Deterministic/ramp cycle by private turn count plus one normal roll per turn. | source-backed/partial | Local helper now mirrors source Roar, Nom no-repeat/turn-count hit scaling, Drool Strength, Slam damage, and source move bytes. Remaining work is trace-backed action-order validation and visual-only randomness proof. |
| Giant Head | Has countdown/ramp behavior; random speech uses `MathUtils`, and `getMove` consumes one normal roll before using source history guards. | source-backed/partial | Local helper now mirrors Glare/Count roll table, A18 shortened countdown, It Is Time damage ramp/cap, source move bytes, fixed A8 HP, and Slow setup marker. Remaining work is executable Slow per-card damage amplification and trace-backed action-order validation. |
| Nemesis | `getMove` uses several target `aiRng.randomBoolean` branches and cooldown state. | source-backed/partial | Local helper now mirrors first move, Scythe/Burn/Tri-Attack thresholds, replacement boolean draws, source move bytes, fixed A8 HP, A18 Burn count, and post-turn Intangible damage cap. Remaining work is trace-backed proof of exact Intangible decrement/action timing and broader action ordering. |
| Reptomancer | Constructor HP uses `monsterHpRng.random(180, 190)`; `getMove` uses target `aiRng.random` recursive rerolls; summons Daggers. | source-backed/partial | Local encounter composition, constructor/`setHp` HP stream, fixed first Spawn Dagger, later move table, `canSpawn` fallback, recursive replacement draw ranges, and Dagger summon HP/opening AI are now source-shaped. Remaining gaps are exact summon slot/action ordering, death cleanup, and Snake Strike's attack+Weak execution surface. |
| Dagger / SnakeDagger | Constructor HP uses `monsterHpRng.random(20, 25)`; fixed Wound then Explode moves. | source-backed/partial | Local entry and Reptomancer summons consume Dagger HP/opening AI rolls, record fixed Wound first move, and Explode now attacks then kills the dagger without a follow-up AI roll. Remaining work is longer-turn/summon-slot trace validation. |
| Deca | Deterministic alternating beam/block pattern; target queues `RollMoveAction` and ignores the roll value. | source-backed/partial | Local Donu/Deca boss pair now constructs Deca with source HP/Artifact and opening Beam after one ignored AI roll, records source move byte `0`, and executes Square as all-living-monster block plus A19 Plated Armor. Remaining gaps are Dazed insertion, exact post-turn action details, and trace validation. |
| Donu | Deterministic alternating strength/beam pattern; target queues `RollMoveAction` and ignores the roll value. | source-backed/partial | Local Donu opening is now Circle/strength after one ignored AI roll, alternates to Beam, and records source move bytes `2`/`0`. Remaining gaps are exact strength-all action ordering, pair interaction traces, and longer-turn validation. |
| Awakened One | Phase/rebirth and summon state driven. | representative/partial | Local representative exists. Cultist summons, Curiosity, rebirth, and stream consumption are not parity-backed. |
| Time Eater | `getMove` uses target `aiRng.random`, `aiRng.randomBoolean(0.66)`, and recursive rerolls; phase/heal threshold and card-count power matter. | representative/partial | Local representative exists. This is a high-risk parity gap because both AI stream and combat hooks are complex. |
| Writhing Mass | `getMove` uses many target `aiRng.random` and `aiRng.randomBoolean` recursive branches; reactive reroll on HP damage matters. | representative/partial | Local representative exists. Exact reactive intent rerolls are not modeled and will desync `aiRng`. |
| Corrupt Heart | `getMove` uses target `aiRng.randomBoolean` to choose first attack in the attack pair after debuff; Beat of Death/Invincible are special powers. | representative/partial | Local representative exists. Exact AI, buff cycle, Beat of Death, Invincible, and stream draws are major gaps. |
| Spire Shield | `getMove` and orb-related constructor/setup use target `aiRng.randomBoolean`; pair positioning/surrounded matters. | representative/partial | Local representative exists. Exact opening, artifact/block, random branch, and positioning are not parity-backed. |
| Spire Spear | `getMove` uses target `aiRng.randomBoolean` branches; Burn insertion matters. | representative/partial | Local representative exists. Exact pair AI and Burn placement are incomplete. |

## Deep Source Pass: Exordium

This section is the beginning of the stricter audit pass. It records
source-derived RNG and move-selection facts from each Exordium Java class, then
compares them to the current local implementation. `RollMoveAction` should be
read as the target action that calls `rollMove()` and therefore consumes one
target `aiRng.random(99)` before passing the result to `getMove(int num)`.

| Decompiled class | Target source-derived behavior | Local comparison / concrete difference |
| --- | --- | --- |
| `AcidSlime_S` | Constructor has fixed max HP in the class; no `monsterHpRng` constructor draw in this class body. The initial `RollMoveAction` consumes one `aiRng.random(99)`. Below A17, `getMove(int num)` ignores `num` but consumes `AbstractDungeon.aiRng.randomBoolean()` to choose Tackle versus Weak. At A17+, empty history sets Weak without consuming that boolean; only the last-two-attack guard can force Tackle. `takeTurn` uses direct `setMove` follow-ups rather than queuing `RollMoveAction`. | Local collapses all Acid Slime sizes into `ACID_SLIME_ID`, but the small-slime entry path now matches this source stream shape: A16 and below consume the integer plus boolean, while A17+ consumes only the ignored integer and opens Weak. Local follow-up uses direct Tackle/Weak alternation without a roll. Remaining gap is trace-backed action ordering plus split/poison edge validation under the collapsed id. |
| `AcidSlime_M` | Constructor takes HP from caller. `takeTurn` always queues `RollMoveAction` after Attack/Weak/Wound. `getMove` uses the passed roll thresholds plus extra `aiRng.randomBoolean` only when move-history constraints force a replacement: A17+ uses 0.6/0.6/0.4 branch probabilities; lower ascension uses 0.5/default and 0.4 branches. | Local has `target_acid_slime_entry_intent_from_roll` and `target_medium_acid_slime_next_intent_from_roll`, so the broad move table is source-shaped. Remaining risk is exact collapsed-size detection by HP and exact extra boolean draw count in every history branch. |
| `AcidSlime_L` | Constructor takes HP from caller. `takeTurn` queues `RollMoveAction` after normal moves, but Split spawns two `AcidSlime_M` children using only `MathUtils.random(-4.0f, 4.0f)` for visual y-offsets. `damage` can interrupt current intent to Split at half HP without an AI draw. `getMove` is roll-threshold based with extra `aiRng.randomBoolean` only in history-forced replacement branches, including last-two Wound/Normal Tackle guards. | Local large helper now takes move history instead of only previous intent, and focused coverage pins zero extra draw for unguarded branches versus one replacement draw for repeated Wound/Normal Tackle guards. Remaining risk is exact split child action ordering/intent timing and broader trace-backed validation. |
| `ApologySlime` | Not ordinary route content. Constructor calls `AbstractDungeon.monsterHpRng.random(8, 12)`. `getMove` uses `AbstractDungeon.aiRng.randomBoolean()` to choose Attack or Lick/Weak. | No local gameplay monster. If modeled later, it needs one `monsterHpRng` HP draw before AI and one `aiRng` boolean per move choice, not the normal Acid Slime collapsed helper. |
| `Cultist` | `getMove` has a `firstMove` guard: first call sets Incantation and returns, ignoring `num`; later calls always set Attack. `takeTurn` queues `RollMoveAction` after both Incantation and Attack, so target still consumes the normal roll before the first fixed Incantation even though `getMove` ignores it. No gameplay `AbstractDungeon.*Rng` calls appear in the class body. | Earlier broad note suspected an extra opening draw; source pass corrects that: because target uses `RollMoveAction`, local's one opening `random_int(99)` is probably aligned even though the first move is fixed. Local should still avoid any extra branch draws; current local surface is Ritual then Attack. |
| `FungiBeast` | `usePreBattleAction` applies Spore Cloud 2; no Artifact setup appears in the decompiled class. `takeTurn` queues `RollMoveAction`. `getMove` uses `num < 60` for Attack versus Grow, with history guards: if two Attacks, choose Grow; if previous Grow, choose Attack. A17+ Grow applies `strAmt + 1`. It contains no extra `aiRng.randomBoolean` calls. | Local has `target_fungi_beast_next_intent_from_roll` and entry/turn code calls it. Focused coverage pins one-roll move shape, A17 Grow +1, Spore Cloud setup/no Artifact, and Spore Cloud release only when the battle is not ending. Remaining gap is trace-backed action ordering and broader HP/routing evidence. |
| `GremlinFat` | Fixed move table: `getMove` always sets Attack+Weak; escape state sets move 99. No gameplay `AbstractDungeon.*Rng` calls in class body. It can queue `RollMoveAction`, but `getMove` ignores `num`. | Local minion exists. Any generic one-roll-per-turn AI consumption is probably target-aligned if target queues `RollMoveAction`, but local should not branch on the roll. Group composition comes from `MonsterHelper`/`miscRng`, not this class. |
| `GremlinNob` | `getMove` first uses Bellow once and returns, ignoring `num`; later A18+ uses deterministic history constraints, while lower ascension uses `num < 33` for Skull Bash versus Rush and history guards. No extra `aiRng.randomBoolean` calls inside `getMove`. | Local has `target_gremlin_nob_next_intent_from_roll`. Opening one `random_int(99)` is target-shaped because source queues `RollMoveAction` before fixed Bellow. Verify local A18 behavior: target A18 removes the `num < 33` branch and prioritizes Skull Bash unless the previous two move slots forbid it. |
| `GremlinThief` | `getMove` always Attack; escape state sets move 99. No gameplay `AbstractDungeon.*Rng` calls in class body. | Local minion exists. AI stream should be at most the normal ignored `RollMoveAction` draw when a roll action is queued; exact escape timing on leader death remains group/action behavior, not local move choice. |
| `GremlinTsundere` | `getMove` always Protect/Defend; escape state sets move 99. In `takeTurn`, Protect queues `GainBlockRandomMonsterAction`, which builds valid targets excluding source, escaping, and dying monsters, then calls `AbstractDungeon.aiRng.random(validMonsters.size() - 1)` if any target exists, otherwise targets self. The class directly sets the next move after Protect/Bash instead of queuing `RollMoveAction`. | Local now mirrors the target-selection stream and direct follow-up shape: Protect consumes combat `aiRng` only for the block target, including one-candidate groups, then records the next Protect/Bash move without a second AI roll. Remaining caveat is exact escape/death-react behavior in Gremlin Leader fights. |
| `GremlinWarrior` | `usePreBattleAction` applies Angry at A17+. `getMove` always Attack; escape state sets move 99. No gameplay `AbstractDungeon.*Rng` calls in class body. | Local attack/anger surface exists. Stream risk is limited to whether local consumes any unnecessary branch draw beyond the normal roll action, and group composition remains `miscRng`. |
| `GremlinWizard` | `getMove` always starts/continues Charging. `takeTurn` uses direct `setMove`: charge increments `currentCharge`, the third charge prepares Dope Magic; Dope Magic resets charge, then below A17 returns to Charging while A17+ keeps attacking. Escape state sets move 99. No gameplay `AbstractDungeon.*Rng` calls in class body. | Local post-turn prep now avoids generic AI rolls and mirrors the charge/charge/blast cycle plus A17 repeated blast behavior. Remaining work is exact escape/death-react behavior and trace validation. |
| `Hexaghost` | `usePreBattleAction` applies Invincible and sets Divider damage from player HP before the fight. `getMove` first sets Activate/Unknown and then follows `orbActiveCount` cycle: Sear, multi-hit Tackle, Sear, Strengthen, Tackle, Sear, Inferno. No gameplay `AbstractDungeon.*Rng` calls in `getMove`; card/status additions are deterministic actions, while visual/orb helpers may use non-gameplay randomness. | Local boss is representative and omits major details. Important correction: target still uses `RollMoveAction` after several turns, but the roll is ignored by the deterministic cycle. Local generic one-roll-per-turn may be stream-shaped, but local must match the exact number of roll actions across Activate, Divider, Sear/Tackle/Strengthen/Inferno, and it currently does not prove that. |
| `HexaghostBody` | Support/visual body class, not an independent combat AI monster in this audit. No ordinary local monster expected. | No local monster needed unless visual/support state is modeled for trace parity, which is outside gameplay simulator scope. |
| `HexaghostOrb` | Support/visual orb class, not an independent combat AI monster in this audit. | No local monster needed for gameplay. |
| `JawWorm` | `usePreBattleAction` grants horde block/strength for multi-Jaw Worm encounters. `getMove` first move is fixed Chomp and ignores `num`; later uses thresholds `<25`, `<55`, and `else`, with extra `AbstractDungeon.aiRng.randomBoolean(0.5625f)`, `0.357f`, or `0.416f` only when history guards force a replacement. | Local has `target_jaw_worm_next_intent_from_roll`, and focused tests now pin the draw-count surface: unguarded threshold branches consume no extra draw, while the three guarded branches each consume exactly one replacement boolean. Opening one roll before fixed Chomp appears target-shaped. Remaining risk is horde pre-battle state plus broader trace-backed action ordering. |
| `Lagavulin` | `getMove` while asleep sets Sleep. Once out, it alternates between two attacks and a Siphon/debuff based on `debuffTurnCount` and history; no gameplay RNG calls in `getMove`. Damage can interrupt Sleep into Stun and later roll movement. | Local sleep/wake/siphon exists. Opening generic roll may be target-shaped only if target queues `RollMoveAction` for the sleeping intent path; exact wake/stun/action-queue timing needs deeper action audit. No local branch randomness should be used for the awake cycle. |
| `Looter` | `getMove` always opens with Swipe/Attack. Subsequent movement is mostly set directly in `takeTurn`: the first Mug consumes `AbstractDungeon.aiRng.randomBoolean(0.6f)` only for speech, then sets Mug again; after the second slash it calls `AbstractDungeon.aiRng.randomBoolean(0.5f)` to choose Smoke Bomb/Defend versus Lunge. Death and voice use `MathUtils` only. | Local turn prep now has a direct-transition helper for Looter, avoiding the old normal `random(99)` roll path after attacks and consuming the source booleans in the same sequence. Remaining work is trace-backed stolen-gold/reward and escape validation. |
| `LouseDefensive` | Constructor/private setup uses `monsterHpRng`: bite damage is `random(6,8)` at A2+ else `random(5,7)`; Curl Up amount is `random(9,12)` at A17+, `random(4,8)` at A7+, else `random(3,7)`. `getMove` uses the passed roll and history only; no extra AI booleans. | Local records rolled attack damage and rolled Curl Up for ordinary louse groups. Mixed Exordium Thugs/Wildlife now also roll Curl Up only when the selected weak monster is a louse, after source candidate constructor/private HP draws. Remaining risk is trace-backed action-order/state-import validation. AI draw shape is one integer per roll action. |
| `LouseNormal` | Same RNG shape as defensive louse: constructor/private bite and Curl Up use `monsterHpRng`; `getMove` uses the passed AI roll and history only. Red/normal Curl Up is a buff instead of defensive louse Weak. | Same source-backed local surface as defensive louse: rolled HP, bite, and delayed Curl Up are represented for ordinary and mixed Exordium louse spawns. Remaining risk is trace-backed action-order/state-import validation. |
| `Sentry` | `getMove` first move depends on monster index parity in the group: even index Debuff, odd index Attack. Later it alternates: after Attack choose Debuff, otherwise Attack. No gameplay RNG calls in `getMove`. | Local Sentry behavior exists. Opening generic `random_int(99)` may be target-shaped if source queues `RollMoveAction`, but local correctness depends on matching index parity and roll-action count, not on the random value. |
| `SlaverBlue` | `getMove` uses the passed roll: if `num >= 40` and not last two Stabs, choose Stab; otherwise choose Rake/Weak unless history forces Stab. At A17+, only last Rake is forbidden; below A17, last two Rakes are forbidden. No extra AI booleans. | Local `target_slaver_blue_next_intent_from_roll` is a good structural match. Verify exact A17 history guard and A2/A17 damage/debuff amounts; stream shape is one AI integer per move. |
| `SlaverRed` | First turn fixed Stab and ignores `num`. Later, `num >= 75` and unused Entangle chooses Entangle; if Entangle already used and `num >= 55` with history allowance, choose Stab; otherwise Scrape/Vulnerable subject to A17 history rules. No extra AI booleans. | Local has `target_slaver_red_next_intent_from_roll`, but `run/map.rs` entry path currently imports/calls Blue Slaver and other helpers and did not show Red Slaver in the initial-entry branch in the inspected snippet. This is a concrete opening-intent risk: Red Slaver first turn should be fixed Stab while consuming only the target roll action. |
| `SlimeBoss` | `getMove` first turn fixed Goop Spray/Sticky and returns. Split is triggered by damage at half HP and spawns one large Spike Slime and one large Acid Slime; split action itself uses no gameplay RNG in the source snippet. The shown `getMove` contains no random branch and no followup branch after first turn; subsequent moves are largely set directly by `takeTurn`/actions. | Local Slime Boss is representative. Stream risk is not just probabilities: local must match direct set-move transitions, half-HP interrupt, and child spawn roll timing. Any normal roll-table AI for Slime Boss would be suspect. |
| `SpikeSlime_S` | `takeTurn` queues `RollMoveAction` after the fixed attack; `getMove(int num)` ignores `num` and always sets Attack. | Local collapsed `SPIKE_SLIME_ID` uses HP to identify small slime, consumes the source-shaped opening AI roll, and ignores its value. Focused coverage pins entry behavior; remaining caveat is broader trace-backed validation for action order, poison, and split-routed small slimes. |
| `SpikeSlime_M` | `getMove` uses `num < 30` for Attack+Slimed versus Frail, with history guards; no extra `aiRng.randomBoolean` calls. Below A17, repeated Frail is guarded by `lastTwoMoves(4)`; at A17+ the guard is `lastMove(4)`. | Local `target_medium_or_large_spike_slime_next_intent_from_roll` matches those history guards, and focused coverage pins the sub-A17 versus A17 split plus medium damage/count/Frail. Remaining risk is broader action-order trace validation. |
| `SpikeSlime_L` | Same move table shape as medium Spike Slime, but split triggers at half HP and spawns two medium Spike Slimes using only `MathUtils.random(-4.0f, 4.0f)` for visual y-offsets. Split child constructors receive current HP as `newHealth`; split intent interrupt consumes no gameplay RNG in the shown code. | Local large helper now applies large Frail amount (2, or 3 at A17+), collapsed generic adaptation uses `max_hp` for large identity, and split children use current HP as max HP. Remaining risk is exact child spawn action ordering/intent timing in longer traces. |
| `TheGuardian` | `getMove` is deterministic: if open/defensive, Charge Up; otherwise Twin Smash attack. Mode-shift damage threshold directly sets Close Up and creates intent. `takeTurn` methods set the following move explicitly: Fierce Bash -> Vent Steam -> Whirlwind -> Charge Up -> Fierce Bash, and defensive Close Up/Twin Smash sequence. No gameplay RNG in the shown `getMove`/state transition code. | Local Guardian is simplified but has mode behavior. Randomness should not drive the sequence; local stream parity depends on matching each target `RollMoveAction` count and direct `setMove` transition, plus the damage-threshold interrupt. Current generic roll consumption alone is not sufficient proof. |

## Deep Source Pass: City

| Decompiled class | Target source-derived behavior | Local comparison / concrete difference |
| --- | --- | --- |
| `BanditBear` | Event fight script uses direct `SetMoveAction` transitions between Bear Hug, Lunge, and Maul-style attacks. No gameplay `AbstractDungeon.*Rng` calls were found in the class body. | Local event monster is representative. This should be modeled as direct scripted transitions, not a normal AI roll table. |
| `BanditLeader` | Event fight script uses direct `SetMoveAction` transitions for Mug/Attack/Vulnerable-style actions. No gameplay `AbstractDungeon.*Rng` calls were found in the class body. | Local representative coverage should not consume AI RNG for branch selection unless the target event controller does so elsewhere. |
| `BanditPointy` | Event fight script uses direct `SetMoveAction`; `getMove` is fixed. No gameplay `AbstractDungeon.*Rng` calls were found. | Local representative Pointy should be deterministic with zero gameplay branch draws. |
| `BookOfStabbing` | `takeTurn` queues `RollMoveAction`. `getMove` uses `num < 15` to choose single stab versus multi-stab, with history guards. `stabCount` increments on multi-stab, and at A18+ it also increments when selecting single stab in the `num < 15` branch. No extra AI RNG calls inside `getMove`. | Local combat entry and turn prep now consume one AI integer per roll action and route through a source-style helper that mutates stored hidden `stabCount`, including the A18 Big Stab increment rule and source move bytes. Remaining gap is trace-backed action-order validation. |
| `BronzeAutomaton` | `getMove` first turn sets Spawn Orbs/Unknown and returns. On turn 4 it sets Hyper Beam and resets `numTurns`. After Hyper Beam, A19+ uses DEFEND_BUFF, otherwise STUN. Other transitions alternate multi-hit attack and DEFEND_BUFF. `MathUtils.randomBoolean` in orb-slot visuals is non-gameplay. No gameplay `AbstractDungeon.aiRng` branch appears in `getMove`. | Local boss turn prep now consumes the normal ignored AI integer and uses a source-shaped helper for Spawn Orbs, Flail, Boost, Hyper Beam, post-beam Stun/A19 Boost, source move bytes, and Boost scaling. Remaining gaps are exact action-order trace validation and death cleanup for live Bronze Orbs. |
| `BronzeOrb` | Constructor calls `AbstractDungeon.monsterHpRng.random(52, 58)`, then `setHp(54, 60)` at A9+ or `setHp(52, 58)` otherwise. `takeTurn` queues `RollMoveAction`. `getMove`: if Stasis unused and `num >= 25`, choose Stasis and mark used; else if `num >= 70` and not last two Supports, choose Defend; else if not last two Beams, choose Attack; else Defend. | Local Bronze Automaton summons now consume both HP rolls and one opening AI integer per orb, and combat entry/turn prep route through the Bronze Orb roll/history helper. Stasis uses `card_random_rng`; exact card-selection priority and trace proof remain. |
| `Byrd` | `getMove` is roll/history/flight-state based and has extra `aiRng.randomBoolean` replacements with probabilities 0.375, 0.4, 0.375, and 0.2857 in the inspected flying branches. When grounded, `getMove` sets Headbutt regardless of roll. Headbutt directly sets Go Airborne and does not queue `RollMoveAction`; Go Airborne reapplies Flight and then queues the next roll. | Local uses `target_byrd_next_intent_from_roll` for flying entry/turn prep, fixes grounded prep to Headbutt after only the normal roll integer, and models Headbutt as a direct no-roll transition to Go Airborne/source byte `2`. Remaining risk is trace-backed proof for damage-triggered Stun/Flight ordering and multi-Byrd action sequencing. |
| `Centurion` | `takeTurn` queues `RollMoveAction`; Protect queues `GainBlockRandomMonsterAction`, which consumes `AbstractDungeon.aiRng.random(size - 1)` to choose a valid non-source ally when one exists and falls back to self otherwise. `getMove` is roll/history and ally-state dependent. | Local helper considers living monster count, and Protect now consumes the combat AI stream for target selection before the normal roll action. Focused coverage pins Protect versus Fury, Slash fallback, and the full-turn RNG count in the Centurion/Mystic pair. Remaining gap is trace-backed action-order/death-escape validation. |
| `Champ` | `takeTurn` queues `RollMoveAction`; `getMove` is phase/state driven. `MathUtils.randomBoolean` appears in dialogue/visual style code, not gameplay move choice in the inspected output. | Local Champ is representative. It should not use random branch draws beyond target roll actions; phase threshold, Execute, Defensive Stance, and Anger transitions need full source-backed modeling. |
| `Chosen` | `takeTurn` queues `RollMoveAction`; no extra gameplay RNG calls found in inventory. `getMove` A17+ fixes first Hex, below A17 fixes first Poke then one Hex, and later branches by `num < 50`, last Debilitate/Drain guards, and `num < 40` Zap fallback. | Local helper is used at combat entry/turn prep and now has focused coverage for the source first-turn, one-time Hex, threshold, history guard, and source byte behavior. Remaining gap is broader trace-backed action-order validation. |
| `GremlinLeader` | `takeTurn` queues `RollMoveAction`. Encourage quote selection uses `AbstractDungeon.aiRng.random(0, list.size() - 1)`. `getMove` branches by alive gremlin count and may recursively call `getMove(AbstractDungeon.aiRng.random(50, 99))` or `getMove(AbstractDungeon.aiRng.random(0, 80))` when history blocks a selected move. | Local combat entry and turn prep now pass the live AI stream into the helper, and focused coverage pins both recursive replacement ranges. Remaining high-risk stream work is exact summon identity/slot/action ordering, Encourage quote draw placement, and minion death/escape trace validation. |
| `Healer` | `takeTurn` queues `RollMoveAction`. Inventory found only `MathUtils.randomBoolean` for non-gameplay animation/dialogue. `getMove` sums missing HP across living non-dying/non-escaping monsters, heals above 15 missing HP or above 20 at A17+ unless last two heals, attacks on `num >= 40` unless history-blocked (last move at A17+, last two below), otherwise buffs all living monsters. | Local helper considers total missing HP and focused coverage pins heal thresholds, A17 history differences, attack+Frail, and Strength-all fallback. Remaining gap is trace-backed action-order/heal-cap validation. |
| `Mugger` | Like Looter but Act 2 variant. `takeTurn` uses `AbstractDungeon.aiRng.random(2)` for attack voice on Mug and Big Swipe, `randomBoolean(0.6f)` for second-Mug speech, and `randomBoolean(0.5f)` after the second slash to choose Smoke Bomb/Defend versus Big Swipe. Death voice also uses `aiRng.random(2)`. `getMove` itself opens with Attack. | Local turn prep now has a direct-transition helper for live Mugger attacks, avoiding the old normal `random(99)` roll path and consuming attack voice plus branch draws in source order. Remaining work is death-voice stream behavior, stolen-gold/reward validation, and trace coverage. |
| `ShelledParasite` | First move: at A17+ fixed Attack+Debuff; below A17 consumes `AbstractDungeon.aiRng.randomBoolean()` to choose double attack versus attack+buff. Later `getMove` uses roll thresholds and recursively calls `getMove(AbstractDungeon.aiRng.random(20, 99))` if a repeat Attack+Debuff is blocked. | Local has a helper with the source first-move boolean, A17+ fixed Fell, and recursive replacement draw path. Remaining gap is trace-backed action-order validation. |
| `SnakePlant` | `takeTurn` queues `RollMoveAction`; no extra gameplay RNG calls found. `getMove`: `num < 65` attacks unless last two attacks, high rolls Spores unless Spores history blocks it; A17+ blocks Spores if either of the prior two move slots was Spores, while lower ascension only blocks the last move. `usePreBattleAction` applies Malleable; Spores applies Frail 2 and Weak 2. | Local helper is used at combat entry/turn prep and focused coverage pins thresholds, A17 guard, Malleable 3 setup, and Spores debuffs. Remaining gap is broader trace-backed action-order validation. |
| `Snecko` | `takeTurn` queues `RollMoveAction`; no extra monster AI RNG calls found. `getMove` fixes first-turn Glare/Confusion, then uses `num < 40` for Tail attack+debuff, otherwise Bite unless the last two moves were Bite. Confusion/card-cost randomness belongs to card/random-cost handling, not `getMove`. | Local combat entry and turn prep route through `target_snecko_next_intent_from_roll`; focused coverage pins the fixed opener, threshold, Bite history guard, and A17 Weak. Remaining gap is broader trace-backed action-order/card-cost validation. |
| `SphericGuardian` | `usePreBattleAction` applies Barricade, Artifact 3, then 40 block. `takeTurn` queues `RollMoveAction`; `MathUtils.randomBoolean` appears only in sound/dialogue. `getMove` first sets move byte `2`/Defend, second sets byte `4`/Attack+Frail, then sets byte `3`/Attack+Defend only after byte `1` big attack, otherwise byte `1` two-hit attack. | Local setup and turn prep now consume one normal AI roll after each turn, ignore the roll value like source, and record source bytes `2/4/1/3`. Remaining gap is broader trace validation of exact queued action ordering and non-gameplay sound randomness. |
| `Taskmaster` | Constructor calls `AbstractDungeon.monsterHpRng.random(54, 60)`, then `setHp(57, 64)` at A8+ or `setHp(54, 60)` otherwise. `takeTurn` queues `RollMoveAction`; `getMove` is fixed Scouring Whip. | Local City member spawn now consumes both HP rolls, and combat entry consumes one ignored AI roll while fixing Scouring Whip and recording source move byte `2`. Remaining gap is broader event/elite trace validation, not the basic stream shape. |
| `TheCollector` | `takeTurn` queues `RollMoveAction`; no gameplay RNG calls found in the class inventory. Move choice depends on torch-head/minion state, history, and boss rules. Torch Head construction handles HP RNG separately. | Local helper exists. Needs exact summon slots, Torch Head HP draw timing, minion-death state, and boss phase/history rules. |
| `TorchHead` | Constructor calls `AbstractDungeon.monsterHpRng.random(38, 40)`, then `setHp(40, 45)` at A9+ or `setHp(38, 40)` otherwise. Uses direct `SetMoveAction` to fixed Attack; `getMove` is fixed. `SpawnMonsterAction` calls `init()`, so the fixed move still consumes one ignored `aiRng.random(99)`. | Local Collector-spawned Torch Heads now consume constructor plus `setHp` HP rolls, consume one ignored spawn-init AI roll, and record move byte `1`. Remaining gap is exact replacement-slot ordering in traces. |

## Deep Source Pass: Beyond

| Decompiled class | Target source-derived behavior | Local comparison / concrete difference |
| --- | --- | --- |
| `AwakenedOne` | `takeTurn` queues `RollMoveAction`. Form 1 first turn is fixed Attack 20 and ignores `num`; later `num < 25` chooses Soul Strike unless history blocks it, otherwise Attack. On death/phase transition, source directly sets move 3 Unknown, clears most debuffs, flips `form1 = false`, and resets `firstTurn = true`. Form 2 first turn is fixed Dark Echo 40, then `num < 50` chooses Sludge unless history blocks it, otherwise multi-hit attack. No gameplay `AbstractDungeon.*Rng` calls found in `getMove`. | Local Awakened One is representative. Stream parity requires exact ignored-roll counts across both phases and the direct rebirth `SetMoveAction`; local needs exact Curiosity/Unawakened/Shackled, cultist handling, and form transition before parity claims. |
| `Darkling` | Constructor/private setup rolls `nipDmg` with `monsterHpRng.random(9, 13)` at A2+ else `random(7, 11)`. `takeTurn` queues `RollMoveAction`. `getMove`: half-dead sets BUFF/reincarnate. First move uses `num < 50` for Defend/Buff or Nip. Later `num < 40` can choose double attack only for even group index and not after last same move, otherwise recursively calls `getMove(AbstractDungeon.aiRng.random(40, 99))`; `num >= 70` has last-two Nip guard and may recurse with `random(0, 99)`. | Local has darkling spawn/HP/private and `target_darkling_next_intent_from_roll_with_rng` in turn prep. Half-dead Count/Reincarnate now records source bytes `4`/`5`, consumes the normal roll after Count, revives to half max HP, and rolls the next move. Remaining gaps are exact all-Darklings-dead room resolution, Regrow power visibility, and broader multi-Darkling trace validation. |
| `Deca` | Constructor uses 250 HP or 265 at A9+, and pre-battle applies 2 Artifact or 3 at A19+. `takeTurn` queues `RollMoveAction`, but `getMove` ignores `num`: if attacking, set two-hit attack+debuff with move byte `0`; otherwise at A19+ set defend+buff, else defend, with move byte `2`. No gameplay `AbstractDungeon.*Rng` calls found beyond the normal roll action. | Local Donu/Deca boss pair now constructs Deca first with source HP/Artifact and opening Beam after one ignored AI roll; source move bytes are recorded; Square applies block and A19 Plated Armor to all living monsters. Remaining parity depends on exact alternating `isAttacking`, Dazed insertion action, and longer trace validation. |
| `Donu` | Constructor uses 250 HP or 265 at A9+, and pre-battle applies 2 Artifact or 3 at A19+. Same ignored-roll pattern as Deca: `getMove` chooses two-hit attack with move byte `0` when `isAttacking`, otherwise Circle/strength with move byte `2`. No gameplay RNG in `getMove` beyond the normal roll action. | Local Donu now opens with Circle/strength, alternates to Beam, has source HP/Artifact in the boss pair, and records source move bytes. Remaining gaps are exact strength-all action execution/order and pair trace validation. |
| `Exploder` | `takeTurn` queues `RollMoveAction`; `getMove` ignores `num` and uses `turnCount`: first two turns Attack, then Explode/Unknown with move byte `2`; pre-battle applies `ExplosivePower(this, 3)`. No gameplay `AbstractDungeon.*Rng` calls found beyond the normal roll action. | Local now consumes the normal AI integer while ignoring its value, follows the two-attack countdown at entry/turn prep, and resolves the third-turn Explosive power as 3 player damage plus self-death with no post-death AI roll. Remaining gap is trace-backed action-order validation. |
| `GiantHead` | `usePreBattleAction` applies Slow and decrements `count` by one at A18+. `takeTurn` queues `RollMoveAction`. Inventory shows `MathUtils` for sound/quotes/visuals only. `getMove` decrements the countdown, chooses Glare on `num < 50` unless last two Glares, chooses Count on high rolls unless last two Counts, and switches to It Is Time once `count <= 1`, ramping by 5 damage per turn up to +30. | Local Giant Head now mirrors fixed HP, Slow setup marker, countdown-derived Glare/Count/It Is Time table, A18 shortened setup, one normal AI integer per roll action, and source bytes. Remaining gap is actual Slow per-card damage amplification plus broader trace validation of queued sound/quote/action ordering. |
| `Maw` | `takeTurn` queues `RollMoveAction`. `getMove` increments `turnCount`, first uses Roar/strong debuff, then if `num < 50` and not last multi-bite uses a scaling multi-hit Bite based on `turnCount / 2`; after last Attack or Bite it buffs; otherwise it attacks. `MathUtils` is visual only. | Local Maw now has a source-shaped one-roll helper for Roar, Nom hit scaling, Drool Strength, Slam, and source bytes. Remaining gap is broader trace validation of queued action order and the visual-only bite-effect randomness. |
| `Nemesis` | `takeTurn` queues `RollMoveAction`. `getMove` decrements `scytheCooldown` first. First move uses `num < 50` for Burn multi-attack versus debuff. Later `num < 30` may choose Scythe if cooldown allows, otherwise uses `aiRng.randomBoolean()` replacement branches; `30 <= num < 65` similarly may use `randomBoolean()` when Burn is history-blocked; high rolls choose debuff unless last debuff, then may `randomBoolean()` into Scythe if cooldown allows. `MathUtils` is sound only. | Local Nemesis now routes combat entry/turn prep through a source-shaped helper that consumes replacement booleans on the same branches, records move bytes, applies fixed HP/Burn counts, and gives Nemesis monster Intangible with damage capping after it acts. Remaining gap is exact Intangible decrement timing and trace validation of queued action order. |
| `OrbWalker` | Constructor calls `monsterHpRng.random(90, 96)`. `takeTurn` queues `RollMoveAction`; no extra gameplay RNG calls found. `getMove` is a simple roll/history table. | Local has a target HP helper but no exact turn-prep helper in the inspected local turn path. Need exact HP draw and move table; stream shape is one HP draw plus one AI integer per roll action. |
| `Reptomancer` | Constructor calls `monsterHpRng.random(180, 190)`, then `setHp(190, 200)` at A8+ or `setHp(180, 190)` otherwise. First move sets summon/Unknown and ignores `num`. Later `num < 33` chooses multi-attack unless last same move, otherwise recurses with `aiRng.random(33, 99)`; `33 <= num < 66` summons if possible and not last two summons, else attacks; high rolls choose big attack unless last, otherwise recurses with `aiRng.random(65)`. | Local now mirrors target encounter composition, HP draw order, fixed first Spawn Dagger intent, later source move table, recursive replacement draw ranges, and Dagger spawn HP/opening AI rolls. Remaining parity work is exact summon slot/action ordering, Reptomancer death cleanup, and Snake Strike's attack+Weak execution surface. |
| `Repulsor` | `takeTurn` queues `RollMoveAction`; no extra gameplay RNG calls found. `getMove`: if `num < 20` and not last Attack, set Attack move byte `2`; otherwise set Daze/debuff move byte `1`. Daze action adds two Dazed cards to the draw pile with random placement. | Local Repulsor now uses the source one-roll table at combat entry and turn prep, records source move bytes, and executes Dazed-to-draw insertion. Remaining work is trace-backed validation of random insertion ordering in multi-shape fights. |
| `SnakeDagger` | Constructor calls `monsterHpRng.random(20, 25)`. `takeTurn` queues `RollMoveAction`; `getMove` is fixed: first Wound attack, then Explode attack. Explode queues a player hit and then `LoseHPAction(this, this, currentHealth)`. No extra gameplay RNG calls found. | Local entry and Reptomancer-spawned Daggers now consume constructor HP/opening AI rolls, ignore the roll value for fixed Wound, and model Explode as attack plus self-death without a follow-up roll. Remaining gap is longer-turn trace validation around Reptomancer summon slots and death ordering. |
| `Spiker` | `usePreBattleAction` applies Thorns 3, 4 by A2, plus +3 at A17+. `takeTurn` queues `RollMoveAction`; buff increments private `thornsCount` and applies +2 Thorns. `getMove` forces Attack when `thornsCount > 5`, otherwise attacks on `num < 50` unless the last move was Attack, else buffs. | Local has `target_spiker_next_intent_from_roll` with the hidden thorns-buff count, source starting Thorns, buff execution, and one AI integer per roll action. Remaining gap is trace-backed action-order validation. |
| `SpireGrowth` | `takeTurn` queues `RollMoveAction`; no extra gameplay RNG calls found. `getMove` is roll/state based: A17+ Constrict first when the player lacks Constricted, Quick Tackle on `num < 50` unless last two tackles, Constrict when missing and not last Constrict, otherwise Smash unless last two Smash, then Quick Tackle. | Local Spire Growth now has the source move table, source bytes, fixed A7 HP, Constricted application, and end-turn HP loss. Remaining gaps are exact source-owner removal when Spire Growth dies and broader action-order trace validation. |
| `TimeEater` | `takeTurn` queues `RollMoveAction`. `getMove` first checks Haste at below half HP and unused; then `num < 45` chooses multi-hit attack unless last two attacks, otherwise recurses with `aiRng.random(50, 99)`. `45 <= num < 80` chooses attack+debuff unless last same, then `aiRng.randomBoolean(0.66f)` chooses multi-hit attack or defend+debuff. High roll chooses defend+debuff unless last, otherwise recurses with `aiRng.random(74)`. | Local Time Eater is representative. High-risk gap: exact recursive replacement ranges, Haste timing, card-count Time Warp hook, and move-history interaction are not parity-backed. |
| `Transient` | No `RollMoveAction` import in inspected output. `getMove` simply sets the current escalating attack; no gameplay RNG calls found. | Local now locks combat entry with no opening AI draw and directly sets the next escalating attack after each turn without consuming `aiRng`; A4+ base damage is 40. Remaining gaps are Fading/Shifting power details and trace validation around death/reward timing. |
| `WrithingMass` | `takeTurn` queues `RollMoveAction`. First move uses `num < 33 / <66 / else` among multi-attack, attack+defend, and attack+debuff. Later branches include many recursive replacements: `random(10, 99)`, `random(20, 99)`, `random(19)`, `random(40, 99)`, `random(39)`, `random(69)`, plus `randomBoolean(0.1f)`, `0.4f`, and `0.3f`. `getMove` calls `createIntent()` at the end. | Local Writhing Mass is representative and does not model the reactive intent reroll on HP damage. This is one of the largest AI stream gaps: replacement recursion and damage-triggered rerolls will desync `aiRng` quickly. |

## Deep Source Pass: Ending

| Decompiled class | Target source-derived behavior | Local comparison / concrete difference |
| --- | --- | --- |
| `CorruptHeart` | `takeTurn` queues `RollMoveAction`. `getMove` first move is fixed Strong Debuff and ignores `num`. Later it cycles by `moveCount % 3`: case 0 consumes `AbstractDungeon.aiRng.randomBoolean()` to choose Blood Shots versus Echo; case 1 chooses the attack not just used, with no RNG; default is Buff. `moveCount` increments after non-first move selection. | Local Heart is modulo-cycle representative. Concrete mismatch: local must consume the Heart attack-pair boolean in exactly case 0 after the first debuff and must model Beat of Death, Invincible, buff/debuff details, and attack history rather than a fixed modulo-only cycle. |
| `SpireShield` | `takeTurn` queues `RollMoveAction`. During Bash/attack, if the player has orbs it consumes `AbstractDungeon.aiRng.randomBoolean()` to choose Focus down versus Strength down; otherwise applies Strength down. `getMove` cycles by `moveCount % 3`: case 0 consumes `aiRng.randomBoolean()` to choose Defend versus Bash; case 1 chooses Bash unless last Bash, otherwise Defend; default is Smash/attack+block. | Local Shield is representative modulo-cycle. It currently lacks exact `aiRng` consumption for both move-choice and orb-sensitive debuff choice, plus pair positioning/surrounded behavior. |
| `SpireSpear` | `takeTurn` queues `RollMoveAction`. `getMove` cycles by `moveCount % 3`: case 0 chooses Burn Strike unless last Burn Strike, otherwise Buff; case 1 Skewer attack; default consumes `AbstractDungeon.aiRng.randomBoolean()` to choose Buff versus Burn Strike. | Local Spear is representative modulo-cycle. Concrete mismatch: the default-cycle boolean draw is not represented, and exact Burn insertion/placement, pair positioning, and Spear/Shield coordination remain incomplete. |

## Encounter / Group RNG Status

`encounters.rs` has source-backed structure for weighted encounter lists:
weak/strong pools, first-strong exclusions, no-repeat rejection, and elite
no-repeat rejection. Prior research pinned Act 1 weak/strong prefixes and boss
list shuffling against decompiled `AbstractDungeon` / `MonsterInfo` behavior.

Remaining group-composition risks are mostly in target `MonsterHelper`:

- `spawnSmallSlimes()` uses `miscRng.randomBoolean`.
- Shape groups use repeated `miscRng.random(shapePool.size() - 1)` draws and
  removals.
- Large slime and gremlin groups use repeated `miscRng` index draws.
- Louse and other helper methods use `miscRng` to pick subtype or member.
- Some helper functions choose a random monster from a list using `miscRng`.

Local spawn helpers cover selected Act 1, City, and Beyond routes, but this is
not a complete proof that every target `miscRng` branch consumes the same number
of draws in the same order.

## Highest-Priority Differences

1. Local `CombatState.monster_rng` is documented in code as combat `aiRng`, but
   a future rename would still reduce confusion. Target `monsterRng` is
   encounter-list generation, not monster AI.
2. Generic combat-entry roll consumption remains risky for fixed-first-move or
   deterministic-cycle monsters such as Cultist, Sentry, Lagavulin, Guardian,
   Hexaghost, Slime Boss, and Bronze Automaton. Torch Head and Donu/Deca now
   have focused source-shaped opening stream coverage, though their longer
   action details still need validation.
   Source-locked initial intents no longer add an extra roll on top of
   helper-produced source state.
3. `monsterHpRng` parity is incomplete beyond early Act 1. Target constructors
   and private fields roll more than just max HP, notably louse bite/Curl Up,
   Darkling Nip and Orb Walker HP. Bronze Automaton-spawned Bronze Orbs,
   Collector-spawned Torch Heads, Taskmaster City spawns, and
   Reptomancer/Dagger entry/summon paths now consume their constructor and
   `setHp` rolls.
4. `miscRng` group composition parity is partial. The simulator should not claim
   every encounter group is exact until `MonsterHelper` methods are translated
   and fixture-tested.
5. Recursive `aiRng` reroll draw counts are high risk for Jaw Worm, Acid Slimes,
   Shelled Parasite, Nemesis,
   Time Eater, Writhing Mass, Corrupt Heart, Shield, and Spear.
6. Status-card placement and random card selection effects need per-action
   stream checks. Bronze Orb Stasis and draw-pile status insertion use
   `card_random_rng` locally, but exact target ordering is not fully audited.

## Recommended Next Audit / Implementation Order

1. Split stream naming in docs/design first: target `monsterRng` versus target
   `aiRng`.
2. Build a compact table of target first-move AI consumption: no draw, one
   `random(99)`, one `randomBoolean`, or recursive draws.
3. Fix combat-entry AI roll timing before tuning individual move probabilities.
4. Translate and fixture-test `MonsterHelper` group constructors for `miscRng`.
5. Audit `monsterHpRng` constructor/private-field draws for every remaining
   class with source snippets and seed-floor fixtures.
6. Continue exact AI helpers in scoped batches. Slaver Red, Snecko, Book of
   Stabbing, Orb Walker, and Bronze Orb now have combat-entry/turn-prep helper
   routing; Bronze Automaton orb summons now seed Bronze Orb opening intents,
   and Bronze Automaton's boss cycle is source-shaped. Remaining scoped
   follow-up includes recursive-reroll monsters plus boss death/action-order
   validation.

## Verification Performed

- Read `AGENT_RULES.md`; this was a documentation-only audit and made no
  simulator behavior changes.
- Listed decompiled monster Java files by scoped package.
- Searched only monster package and `MonsterHelper.java` for gameplay RNG
  sources: `monsterHpRng`, `aiRng`, `miscRng`, `cardRandomRng`, `getMove`, and
  `rollMove`.
- Inspected local monster content ids, executable definitions, target helpers,
  combat-entry stream setup, and monster-turn stream use.
- Added deep source-pass tables for Exordium, City, Beyond, and Ending classes.
- Did not run `cargo test`, because this is documentation-only and no simulator
  code behavior was changed.
