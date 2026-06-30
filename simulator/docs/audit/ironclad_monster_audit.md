# Ironclad Encounterable Monster Audit

Date: 2026-07-01

## Scope

This audit compares every public Spire Archive monster entry that can appear across possible Slay the Spire 1 Ironclad runs against the local simulator state in `simulator/crates/sts_core/src/content/monsters.rs` and encounter routing in `simulator/crates/sts_core/src/content/encounters.rs`. "Encounterable" includes ordinary hallway fights, elites, bosses, event fights, summoned minions, and Act 4/keyed-run monsters; it does not mean that all of these appear in a single run.

The public baseline used for this pass is Spire Archive's STS1 monster data:

- `https://spire-archive.com/api/sts1/monsters?limit=100`
- Individual pages under `https://spire-archive.com/sts1/monsters/<ID>`, for example `https://spire-archive.com/sts1/monsters/GREMLINNOB`

Fandom and wiki.gg were attempted first, but both returned anti-bot challenge pages from non-browser tooling. Spire Archive was accessible and provided structured HP and move data for 66 monsters. This report treats that public data as a comparison baseline, not as a final source-code proof. Existing repo notes that say "source-backed" are retained as local evidence labels, but the findings below are based on current code inspection plus the public API. Spire Archive's `type` field is not used as the final route classification here because several monsters that are ordinary hallway monsters in-game are labeled `Elite` in that API.

## Summary

- Public baseline entries reviewed: 66.
- Local monster content definitions found: 37 game-facing definitions, plus `FIXED_SIMPLE_MONSTER` as a non-public fallback fixture.
- Fully missing ordinary encounter, boss, event, summon, and Ending monsters: 24 public entries have no local `MonsterDefinition`.
- Special/public-list entry: `APOLOGY_SLIME` is present in the public API and has no local `MonsterDefinition`, but is not part of ordinary Ironclad route generation; it is still listed below.
- Common local pattern: many monsters have correct constants and separate `target_*_next_intent_from_roll` helpers. Run-entry and end-of-turn paths use those helpers for several normal/elite monsters when `monster_rng` is present, but fixtures and unsupported branches still fall back to deterministic representative sequences.
- Common HP pattern: many local `MonsterDefinition.hp` values are midpoint fixtures. Range helpers exist for several monsters, but direct executable construction still often uses the fixture plus generic scaling.
- Local run generation caveat: `encounters.rs` names Beyond fights such as Shapes, Spire Growth, Transient, Maw, Giant Head, Nemesis, and Reptomancer, but `target_beyond_encounter_spawn_for_key` only supports `3 Darklings` and `Orb Walker`; unsupported Beyond fights fall back to the initial Cultist fixture when entered through the current run map path.

## High-Confidence Gameplay Differences

These are not merely "coverage missing" rows; they are implemented local behavior that conflicts with the public baseline or is materially narrower than the encounterable monster's behavior.

| Monster | Public baseline | Local behavior | Difference |
| --- | --- | --- | --- |
| Orb Walker | Laser `10` / `11`, Claw `15` / `16` | Laser constant `15`; Claw constant `10`; Claw adds a Burn to discard/draw path | Damage values appear swapped/wrong, and the public move intent labels do not match the local Burn insertion behavior. |
| Hexaghost | Divider dynamic damage with 6 hits; Tackle `5` / `6` x2; Inflame grants block/buff; Sear `6`; Inferno `2` / `3` x6 | Divider fixed `6` x2; no Inflame block/buff; Sear uses Divider damage; Inferno modeled as one `2` damage event plus 3 Burns | Boss cycle and damage shape are substantially incomplete. |
| Slime Boss | Slam `35` / `38`; Preparing; Split; Goop Spray | Always reports Slam intent before split; no Preparing/Goop Spray cycle; split creates two generic Acid Slime states | Major boss phase/move behavior missing. |
| Gremlin Nob | Rush `14` / `16`; Skull Bash `6` / `8`; Bellow buff | Local A0 values only in executable intent; no A3/A18 damage path for Nob attacks | Ascension damage differences missing. |
| Lagavulin | Strong Atk `18` / `20` | Local attack is always `18` | Ascension damage missing. |
| Sentry | Beam `9` / `10` | Local defines `SENTRY_A3_ATTACK_DAMAGE = 10`, but executable `sentry_intent` uses `9` | Ascension damage constant exists but is not wired. |
| The Guardian | Fierce Bash `32` / `36`; Roll Attack `9` / `10`; Whirlwind; Vent Steam; defensive mode | Local uses A0 damage only; defensive cycle simplified; mode-shift behavior exists | Ascension damage and full boss state/move parity incomplete. |
| Bronze Automaton | Flail `7` / `8` x2; Hyper Beam `45` / `50`; Boost block 9; Spawn Orbs | Local uses A0 damage only; spawn action is represented with `SummonGremlins`; fixed orb HPs | Ascension damage and action identity are wrong/incomplete. |
| Bronze Orb | Beam 8; Support Beam block 12; Stasis | Local has Beam and Stasis-like placeholder, then `Block { block: 0 }` | Support Beam block is missing. |
| Mugger | Smoke Bomb public block is 28 | Local escape setup block is 11, 17 at high ascension | Smoke Bomb block differs. |
| Spheric Guardian | Public initial block-gain move lists block 95; Big Attack 10/11 x2; Block Attack 10/11 plus block 15; Frail Attack 10/11 | Local starts with 40 block and opens with 25/35 block, then attack/debuff/double/attack+block loop | Public-data block value does not match local; this row needs source confirmation before changing code. |
| Darkling | Chomp `8` / `9` x2; Harden 12; Nip rolled damage; Count; Reincarnate | Local fixture/default intent is single attack or block; target helper contains richer move selection for RNG-backed combat, but Count/Reincarnate are not executable | Phase/death behavior and fixture/default Chomp shape incomplete. |
| Orb Walker | HP `90-96` / `92-102` | Local fixed HP `96`, generic ascension scaling if constructed with `monster_state_for_ascension` | Local fixture can exceed or miss public range semantics. |

## Full Monster Inventory

Status values:

- `executable`: local `MonsterDefinition` exists and local combat can create a `MonsterState`; this is not a claim that every encounter route, event branch, or boss roll for that monster is wired.
- `partial`: local behavior exists but public moves/phases/ascension/routing are incomplete.
- `metadata/helper`: local target metadata or helper functions exist, but normal executable combat is not fully wired.
- `missing`: no local monster definition was found.
- `special`: public entry is not part of ordinary Ironclad route generation, but is included for completeness.

| Public ID | Name | Act/type | Local status | Differences and notes |
| --- | --- | --- | --- | --- |
| ACIDSLIME_L | Acid Slime (L) | Exordium normal | partial | One local `ACID_SLIME_ID` covers small/medium/large by HP. Large split is represented by `large_acid_slime_on_hp_damage` and `apply_large_acid_slime_split`, but public large Acid Slime has Corrosive Spit `11/12`, Normal Tackle `16/18`, Lick, and Split. Local fixture/default intent starts from the small-slime cycle unless HP/rolled fields route through target helpers. |
| ACIDSLIME_M | Acid Slime (M) | Exordium normal | partial | Public Wound Tackle `7/8`, Normal Tackle `10/12`, Lick. Local constants and target helper cover these better than the fixture/default executable cycle. |
| ACIDSLIME_S | Acid Slime (S) | Exordium normal | executable | Public Tackle `3/4` and Debuff. Local fixture/default small slime alternates Weak then attack `7` through `acid_slime_intent`; the public small Tackle damage `3/4` is represented in `target_acid_slime_entry_intent_from_roll` used by RNG-backed run entry. |
| APOLOGY_SLIME | Apology Slime | Exordium normal/special | special | No local definition. This is in the public API but not in ordinary Ironclad route generation. |
| CULTIST | Cultist | Exordium normal | executable | HP range helpers match public `48-54` / `50-56`; executable fixture uses `50`. Public moves Dark Strike 6 and Incantation. Local Ritual 3 then attack 6 matches A0 surface. |
| GREMLINFAT | Fat Gremlin | Exordium normal/minion | executable | Public Smash `4/5` plus escape. Local minion attack/debuff exists and A17 adds Frail+Weak; escape-on-leader-death is modeled globally. |
| FUNGIBEAST | Fungi Beast | Exordium normal | partial | Public Bite 6 and Grow. Local Bite/Grow plus Spore Cloud are modeled; RNG-backed run paths use the target helper, while fixture/default behavior remains deterministic Bite then Grow. |
| GREMLINNOB | Gremlin Nob | Exordium elite | partial | Public HP `82-86` / `85-90`; local fixture HP `82`. Public Rush `14/16`, Skull Bash `6/8`, Bellow. Local A0 attack values only; Enrage/Anger on player Skills is modeled at 2. |
| GREMLINWIZARD | Gremlin Wizard | Exordium normal/minion | executable | Public Ultimate Blast `25/30` after Charging. Local two charge turns then attack, plus escape-on-leader-death. |
| HEXAGHOST | Hexaghost | Exordium boss | partial | Public boss has dynamic Divider x6, Tackle `5/6` x2, Inflame block/buff, Sear, Activate, Inferno `2/3` x6. Local omits dynamic Divider, Inflame, A-damage, and models Inferno as one small damage event plus Burns. |
| JAWWORM | Jaw Worm | Exordium normal | partial | Public Chomp `11/12`, Bellow block 6, Thrash `7` + block 5. Local fixture/default sequence is deterministic Chomp/Thrash/Bellow; RNG-backed run entry and turn progression use the target helper. A2 Chomp `12` is not wired in the helper/default intent. |
| LAGAVULIN | Lagavulin | Exordium elite | partial | Public attack `18/20`, Siphon Soul, Open, Idle. Local sleep/wake/siphon exists; attack remains `18` across ascensions. |
| LOOTER | Looter | Exordium normal | partial | Public Mug `10/11`, Smoke Bomb block 6. Local A0/A2 swipe and A17 theft are modeled. Local deterministic sequence does not claim exact post-mug branch randomness. |
| FUZZYLOUSEDEFENSIVE | Louse | Exordium normal | metadata/helper | Public defensive louse HP `11-17` / `12-18`, Bite, Spit Web. Local maps louse behavior through red/green content ids with HP/kind helpers; exact public defensive-normal identity is not a distinct executable content id. |
| FUZZYLOUSENORMAL | Louse | Exordium normal | metadata/helper | Public normal louse HP `10-15` / `11-16`, Bite, Grow. Local has red/green content ids plus target louse spawn helpers; default local red/green semantics do not directly mirror public fuzzy normal/defensive ids. |
| GREMLINWARRIOR | Mad Gremlin | Exordium normal/minion | executable | Public Scratch `4/5` plus escape. Local attack and Anger power are modeled; name differs locally as `Gremlin Warrior`. |
| SENTRY | Sentry | Exordium elite | partial | Public Bolt and Beam `9/10`. Local artifact and Beam/Dazed alternation exist, but executable attack uses `9` even when ascension would use `10`. |
| GREMLINTSUNDERE | Shield Gremlin | Exordium normal/minion | executable | Public Protect, Shield Bash `6/8`, escape. Local Protect block and Bash damage variants are modeled; target selection for Protect is simplified. |
| SLAVERBLUE | Slaver | Exordium normal | partial | Public Stab `12/13`, Rake `7/8`. Local has Blue Slaver Stab/Rake with weak variants; fixture/default sequence is deterministic rather than full AI. |
| SLAVERRED | Slaver | Exordium normal | partial | Public Stab `13/14`, Scrape `8/9`, Entangle. Local has all three surfaces; fixture/default order is deterministic. |
| SLIMEBOSS | Slime Boss | Exordium boss | partial | Public includes Slam `35/38`, Preparing, Split, Goop Spray. Local only exposes Slam as default intent and split threshold behavior. |
| GREMLINTHIEF | Sneaky Gremlin | Exordium normal/minion | executable | Public Puncture `9/10` plus escape. Local attack and escape-on-leader-death are modeled; name differs locally as `Gremlin Thief`. |
| SPIKESLIME_L | Spike Slime (L) | Exordium normal | partial | One local `SPIKE_SLIME_ID` covers sizes by HP. Public large has Flame Tackle `16/18`, Lick, Split. Local split for Spike Slime large is not implemented; medium/large HP-driven intent patching is partial. |
| SPIKESLIME_M | Spike Slime (M) | Exordium normal | partial | Public Flame Tackle `8/10`, Lick. Local HP-driven wrapper maps medium attack to `8` but A2 `10` is not in the visible default constants. |
| SPIKESLIME_S | Spike Slime (S) | Exordium normal | executable | Public Tackle `5/6`. Local small Spike Slime attack is `5` and Lick Weak; public small API lists only Tackle, so local extra Weak behavior should be source-checked. |
| THEGUARDIAN | The Guardian | Exordium boss | partial | Public A+ damage and full mode behavior not fully wired. Local mode-shift, defensive mode, Close Up, Roll Attack, Twin Slam, Whirlwind, Charge Up, and Vent Steam are present but simplified and A0-valued. |
| BANDITBEAR | Bear | City event | missing | No local monster definition. Encounterable through Masked Bandits event. |
| BOOKOFSTABBING | Book of Stabbing | City elite | partial | Public Stab `6/7`, Big Stab `21/24`. Local Painful Stabs and growing stab hits exist; deterministic representative sequence instead of full random/history AI. |
| BRONZEAUTOMATON | Bronze Automaton | City boss | partial | Public Flail `7/8` x2, Hyper Beam `45/50`, Stunned, Spawn Orbs, Boost. Local A0-only damage, Artifact 3, representative orb spawn, and `SummonGremlins` action identity mismatch. |
| BYRD | Byrd | City normal | partial | Public Peck 1 x5, Swoop `12/14`, Headbutt 3, Caw, Airborne, Stunned. Local flight and attacks are modeled, but sequence is deterministic and Go Airborne/Stunned loop is simplified. |
| CENTURION | Centurion | City normal | partial | Public Slash `12/14`, Protect, Fury `6/7` x3. Local target helper considers living monster count in RNG-backed combat; fixture/default sequence is deterministic. |
| CHOSEN | Chosen | City normal | partial | Public Zap `18/21`, Drain, Debilitate `10/12`, Hex, Poke `5/6` x2. Local Hex and status/power effects are modeled; RNG-backed run paths use the target helper, while fixture/default behavior is representative. |
| GREMLINLEADER | Gremlin Leader | City elite | partial | Public Rally, Encourage block 6, Stab 6 x3. Local leader/minions are modeled with representative and target summon paths. Exact random minion identity/slot/Rally AI is caveated in existing docs. |
| MUGGER | Mugger | City normal | partial | Public Mug `10/11`, Smoke Bomb block 28. Local theft attacks exist, but escape setup block is `11/17`, not public 28. |
| HEALER | Mystic | City normal | partial | Public Attack `8/9`, Heal, Buff. Local heal/strength/frail attack are modeled; default sequence is deterministic while target helper covers missing HP and roll thresholds. |
| BRONZEORB | Orb | City boss minion | partial | Public Beam 8, Support Beam block 12, Stasis. Local lacks Support Beam's 12 block in default path. |
| BANDITCHILD | Pointy | City event | missing | No local monster definition. Encounterable through Masked Bandits event. |
| BANDITLEADER | Romeo | City event | missing | No local monster definition. Encounterable through Masked Bandits event. |
| SHELLED_PARASITE | Shelled Parasite | City normal | partial | Public Fell `18/21`, Double Strike `6/7` x2, Life Suck `10/12`, Stunned. Local Plated Armor, Fell/Double/Life Suck are modeled; RNG-backed run paths use special first-turn and target-helper logic, while fixture/default behavior is representative. |
| SNAKEPLANT | Snake Plant | City normal | partial | Public Chompy Chomps `7/8` x3 and Spores. Local Malleable and both moves are modeled; RNG-backed run paths use the target helper, while fixture/default behavior is deterministic. |
| SNECKO | Snecko | City normal | partial | Public Glare, Bite `15/18`, Tail Whip `8/10`. Local Confusion, Bite, Tail Whip status variants are modeled; fixture/default sequence is representative. |
| SPHERICGUARDIAN | Spheric Guardian | City normal | partial | Public Big Attack `10/11` x2, Initial Block Gain block 95, Block Attack block 15, Frail Attack. Local starts at 40 block and opens with 25/35 block. Needs source-backed reconciliation. |
| SLAVERBOSS | Taskmaster | City elite/event | partial | Public Scouring Whip 7. Local Scouring Whip exists, but `taskmaster_intent()` ignores ascension wound-count helper in the default intent. Encounterable with Slavers elite and Colosseum. |
| CHAMP | The Champ | City boss | missing | No local monster definition. |
| THECOLLECTOR | The Collector | City boss | missing | No local monster definition. |
| TORCHHEAD | Torch Head | City boss minion | missing | No local monster definition. Summoned by The Collector. |
| AWAKENEDONE | Awakened One | Beyond boss | missing | No local monster definition. |
| DAGGER | Dagger | Beyond elite minion | missing | No local monster definition. Summoned by Reptomancer. |
| DARKLING | Darkling | Beyond normal | partial | Public Chomp `8/9` x2, Harden, Nip, Count, Reincarnate. Local HP/rolled Nip helpers and RNG-backed move helper exist, but executable phase/death behavior and fixture/default Chomp shape are incomplete. |
| DECA | Deca | Beyond boss | missing | No local monster definition. |
| DONU | Donu | Beyond boss | missing | No local monster definition. |
| EXPLODER | Exploder | Beyond normal/shape | missing | No local monster definition. Appears in Shapes encounters. |
| GIANTHEAD | Giant Head | Beyond elite | missing | No local monster definition. |
| NEMESIS | Nemesis | Beyond elite | missing | No local monster definition. |
| ORB_WALKER | Orb Walker | Beyond normal | partial | Public Laser `10/11` and Claw `15/16`. Local constants are Laser `15`, Claw `10`, plus Burn insertion on Claw path. This is the clearest implemented-value mismatch. |
| REPTOMANCER | Reptomancer | Beyond elite | missing | No local monster definition. |
| REPULSOR | Repulsor | Beyond normal/shape | missing | No local monster definition. Appears in Shapes encounters. |
| SPIKER | Spiker | Beyond normal/shape | missing | No local monster definition. Appears in Shapes encounters. |
| SERPENT | Spire Growth | Beyond normal | missing | No local monster definition. |
| MAW | The Maw | Beyond normal | missing | No local monster definition. |
| TIMEEATER | Time Eater | Beyond boss | missing | No local monster definition. |
| TRANSIENT | Transient | Beyond normal | missing | No local monster definition. |
| WRITHINGMASS | Writhing Mass | Beyond normal | missing | No local monster definition. |
| CORRUPTHEART | Corrupt Heart | Ending boss | missing | No local monster definition. Encounterable in a keyed Ironclad run. |
| SPIRESHIELD | Spire Shield | Ending elite | missing | No local monster definition. Encounterable in a keyed Ironclad run. |
| SPIRESPEAR | Spire Spear | Ending elite | missing | No local monster definition. Encounterable in a keyed Ironclad run. |

## Local Surfaces That Need Special Care

### Size-collapsed slimes

The simulator uses one `ACID_SLIME_ID` and one `SPIKE_SLIME_ID` for multiple public-size variants. The code has HP-based wrappers and target helpers for medium/large behavior, but default construction with `ACID_SLIME_A0` or `SPIKE_SLIME_A0` is small-fixture oriented. Any future fix should decide whether to keep size-collapsed content ids with explicit size state or split local content ids to match public/game ids.

### Louse identity

The public baseline has `FUZZYLOUSENORMAL` and `FUZZYLOUSEDEFENSIVE`; local content has `RED_LOUSE_ID` and `GREEN_LOUSE_ID` plus target spawn helpers for louse kind, curl-up, and rolled bite damage. Do not read the local red/green ids as a direct one-to-one public id mapping without checking target game ids and trace fields.

### Target helpers versus executable combat

Several target helpers are closer to public behavior than the fixture/default executable path and are used by some RNG-backed run paths:

- `target_jaw_worm_next_intent_from_roll`
- `target_chosen_next_intent_from_roll`
- `target_snake_plant_next_intent_from_roll`
- `target_centurion_next_intent_from_roll`
- `target_healer_next_intent_from_roll`
- `target_shelled_parasite_next_intent_from_roll`
- `target_gremlin_leader_next_intent_from_roll`
- `target_darkling_next_intent_from_roll`
- slime entry/next-intent helpers

This audit counts a monster as partial when the helper exists but fixture/default behavior, event/boss routing, summon behavior, death phases, or unsupported encounter branches still use representative or missing behavior.

## Recommended Follow-Up Order

1. Fix clear implemented-value mismatches first: Orb Walker damage/actions, Sentry A3 damage wiring, Lagavulin/Nob/Guardian/Bronze Automaton ascension damage.
2. Decide whether slime and louse identity should be split into public/game-aligned content ids or kept collapsed with explicit size/kind fields.
3. Replace representative intent sequencing with target RNG/history helpers where helpers already exist.
4. Add missing Act 2 event/boss monsters: Bear, Pointy, Romeo, Champ, Collector, Torch Head.
5. Add missing Act 3 and Ending monsters only as planned scoped tasks, respecting `AGENT_RULES.md` scope control.

## Verification Performed

- Inspected local monster definitions, constants, target spawn helpers, and intent selection in `simulator/crates/sts_core/src/content/monsters.rs`.
- Inspected local encounter lists in `simulator/crates/sts_core/src/content/encounters.rs`.
- Fetched public baseline from Spire Archive API on 2026-07-01.
- Did not run simulator tests because this change is documentation-only.
