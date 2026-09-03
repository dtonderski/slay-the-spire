# Fair Observation Hidden-State Audit

Status: Lane 3 combat-search audit against schema V2 at epoch
`1de079889eb14f5a86a14073f7ca3dd2ec19d80b`.
Source of truth for projection: `crates/sts_core/src/combat/fair_observation.rs`.
Taxonomy: [`docs/project_history.md`](project_history.md) (fair-belief work is deferred until a calibrated consumer exists) and [`PROJECT_OVERVIEW.md`](../PROJECT_OVERVIEW.md) State Visibility.

This audit classifies every `FairCombatObservation` field and every
combat-search-relevant hidden simulator field. It does not change the
projection. Neural inference remains `FairCombatObservation` plus public
`ActionDescriptor` values. Privileged full-state PUCT remains teacher-only.

## Classification

| Class | Meaning | Belief treatment |
|---|---|---|
| **Public** | Real UI shows it now, or the current schema already projects it with a recorded UI/history justification. | Store in `PublicKnowledge`; reconstruct the public field of a fresh generated rollout. Legal neural input. |
| **Public-history** | A careful player can derive it from earlier UI, but the current observation may omit it. | Compute only from the typed public event prefix. Missing snapshot-only history is refused. |
| **Latent-generative** | Hidden now; a declared prior may sample it without truth. Pile order uses a uniform permutation of the public multiset. Combat RNG in this slice is an independently sampled exchangeable-future approximation, not post-entry reconstruction. | Sample without truth. Observations may condition/reweight; never patch hidden state. |
| **Forbidden** | Hidden with no source-backed prior, or internal identity/queue scaffolding. | Refuse. Internal IDs may be newly allocated deterministically, but are never copied or inferred. |

Exchangeable-randomness convention (architecture §1 seed-divination caveat):
reported agents must not reconstruct `RandomXS128` / `StsRng` streams from
public outcomes. Unrealized randomness is drawn fresh from the named belief
sampler, not from recovered game streams.

## 1. `FairCombatObservation` V2 fields

Schema version constant: `FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION = 2`.

| Field | Class | Evidence | Combat-search note |
|---|---|---|---|
| `schema_version` | Public | Contract version, not game state. | Gate consumers; not a hidden identity. |
| projection error `UnmodeledPublicContent` | Public | Fail-closed for pool identities that exist in the game but have no modeled `CardDefinition`. The public key is shown; internal ids are not. | Added after the previous audit epoch; not a hidden-state leak. |
| `context.ascension` | Public | Run UI / character select. | Conditions A0 vs higher move tables. |
| `context.act` | Public | Act banner, map. | Encounter tables. |
| `context.floor` | Public | Map floor index. | Floor-offset combat RNG is hidden; the floor number is not. |
| `context.gold` | Public | Gold counter. Projection adds `combat_gold_gained` so in-combat gold matches the visible total. | Terminal proxy / potion opportunity cost. |
| `phase` | Public | Combat UI: player turn, enemy turn, victory, death. Coarse `CombatPhase` only. | Fair search should act on `waiting_for_player`. Other phases are not deployable decision roots. |
| `player.hp` / `max_hp` / `block` / `energy` / `max_energy` | Public | Player panel. | |
| `player.powers[]` | Public | Allowlisted visible powers plus displayed Strength/Thorns totals and their public turn-end components (`lose_strength`, `lose_dexterity`, `temporary_thorns`, `rage`, `no_block`, `no_draw`, `double_tap`, `duplication`, `bomb_*`). Sorted by key. | Amount `0` omitted. |
| `orb_slots[]` | Public | Orb panel. Empty slots explicit. V2 additive. | Ironclad is empty unless Prismatic Shard / similar opened slots. |
| `hand[].slot` | Public | Decision-local hand index, not `CardId`. | Policy must be permutation-equivariant with matching choice slots. |
| `hand[].card` | Public | See card table. | |
| `draw_pile.count` | Public | Pile viewer count. | |
| `draw_pile.cards[]` | Public | Unordered multiset; canonical `FairCard` sort. Real pile viewer shows sorted contents, not order. | Analytic remaining-multiset prior. |
| `draw_pile.known_order[]` | Public when a visibility rule holds; otherwise must be empty | Frozen Eye: top-to-bottom full order from draw-pile storage (last vec entry is top). V2 still has no Headbutt/Scry public-history prefix. | Empty ⇒ entire order is latent-generative (uniform permutation). Full length ⇒ order is public; do not reshuffle. |
| `discard_pile.count` / `cards[]` | Public | Inspectable pile; canonical multiset. | |
| `discard_pile.known_order[]` | Forbidden if ever populated from storage order | Producers emit `[]`. Internal `Vec` order is hidden. | Next reshuffle is latent-generative. |
| `exhaust_pile.*` | Same as discard | Inspectable contents; storage order hidden. | |
| `monsters[]` | Public records; see monster table | Visible slots preserved. | |
| `relics[]` | Public identities and allowlisted counters | Visible relic bar order. | Unlisted relics contribute no `state` counters. |
| `potion_slots[]` | Public | Belt slots; empty has no identity. | |
| `selection` | Public overlay | Kind, visible options, selected slots. Hidden-pile options are canonicalized before slots so pile indices cannot leak. | |
| `public_counters[]` | Public-history | `cards_played_this_turn`, `attacks_played_this_turn`, `cards_discarded_this_turn`. UI-derivable this turn. | |

### 1.1 `FairCard`

| Field | Class | Evidence |
|---|---|---|
| `content_key` | Public | Stable definition key, never `ContentId`. |
| `cost` | Public | Effective displayed cost (`-1` = X). Includes Corruption / temp cost / Blood for Blood reduction as displayed. |
| `cost_is_modified` | Public | Differs from printed cost. |
| `cost_resets_next_turn` | Public | Turn-only cost overlay. |
| `upgrade_level` | Public | Instance upgrades not already in the key. |
| `bottled` | Public | Bottled relic marking. |
| `temporary` | Public | Combat-only generated card. |
| `dynamic.rampage_damage_bonus` | Public | Instance text. |
| `dynamic.ritual_dagger_damage_bonus` | Public | Instance text. |
| `dynamic.windmill_retain_damage` | Public | V2; retain upgrade text. |
| `dynamic.steam_barrier_block_reduction` | Public | Instance text. |
| `dynamic.combat_cost_under_turn_override` | Public-history | Combat-long cost that will return when a turn-only overlay expires (Streamline under a zero-cost override). Learnable from the overlay. |

`CardId`, `content_id`, `free_to_play_once` as a separate flag, and
`blood_for_blood_cost_reduction` as a raw integer are **not** projected.
Displayed cost already carries the public effect of those last two.

### 1.2 `FairMonster`

| Field | Class | Evidence |
|---|---|---|
| `slot` | Public | Left-to-right UI slot. |
| `content_key` | Public | Definition `name`, never `MonsterId` / `ContentId`. |
| `slime_size` | Public | Visible slime tier. |
| `hp` / `max_hp` / `block` | Public | Monster panel. Max HP was rolled at spawn; once shown it is public. |
| `powers[]` | Public allowlist | Vulnerable, Weak, Strength, Artifact, Flight, Intangible, Plated Armor, Painful Stabs, Explosive, Ritual, Spikes, Curl Up, Anger, Metallicize, Malleable (displayed amount), Spore Cloud, Strength Up, Slow, Poison, Lock-On, Mark, `restore_strength`, and Mode Shift amount while not in defensive mode. |
| `stolen_gold` | Public | Looter/mugger gold on the monster. |
| `stasis_card` | Public **when present** | V1/V2 project the captured card without instance id. Architecture §1 still lists “stasis identity before reveal” as hidden. The implemented schema treats a present Stasis card as UI-visible (Bronze Orb). This is **not** classified as a leak in the current producers; it is a documented taxonomy mismatch. Confirming jar UI remains a source note, not a projection change in this lane. |
| `intent` | Public except under Runic Dome | Visible category plus displayed per-hit damage (Strength/Weak/Vulnerable adjusted) and hits for attacks. Runic Dome → `hidden` with no category/damage/hits. Dead → `none`. Pending/source-unknown → visible `unknown`. |
| `alive` / `escaped` / `minion` / `targetable` / `in_defensive_mode` | Public | Status flags. `targetable` currently equals `alive`. |

### 1.3 Relic counters actually projected

Allowlist in `project_relic_state`: Lizard Tail availability; Ink Bottle,
Ornamental Fan, Nunchaku, Pen Nib, Shuriken, Kunai, Letter Opener, Happy
Flower, Sundial, Incense Burner, Centennial Puzzle, Akabeko, Pocketwatch, Art
of War, Orange Pellets, Necronomicon used-this-turn, Self-Forming Clay, Red
Skull, Velvet Choker, Horn Cleat / Captain's Wheel / Stone Calendar
(`player_turns_started`), Omamori charges, Maw Bank, Ancient Tea Set, Girya,
Matryoshka, Tiny Chest, Wing Boots, Neow's Lament.

These are UI-visible or public-history.
Generic relic internals are omitted.

### 1.4 Selection kinds

Public overlay kinds include potion/toolbox/discovery rewards, Warcry,
Armaments, Forethought, Thinking Ahead, Prepared, Dual Wield, Secret
Technique/Weapon, Scry, Liquid Memories, Headbutt, Hologram, Exhaust,
Gambling Chip, Exhume, Purity, Burning Pact, True Grit, Recycle.

`NilrysCodexCardReward` is transported as `FairSelectionKind::ToolboxReward`.
That aliases a public screen name; it does not leak hidden offers. It is
over-redaction of the overlay title, not a hidden-state leak.

## 2. Authoritative fields not in the fair observation

These are the combat-search-relevant hidden fields. They may not enter a
belief through a privileged `RunState` scaffold. The Rust materializer either
constructs them from public history, samples them through a declared prior, or
refuses the root. Canonical run-envelope fields are allowed only when they are
provably unreachable before the combat-only horizon ends.

### 2.1 Combat RNG — exchangeable future, never neural input

`CombatRngState` flattened onto `CombatState`: `shuffle_rng`, `monster_rng`
(aiRng), `monster_hp_rng`, `card_random_rng`. Each `StsRng` has `seed0`,
`seed1`, `counter`.

Combat entry derives these streams from floor-adjusted run seeds
(`crates/sts_core/src/run/map.rs` `enter_combat_with_monsters`): shuffle and
monster-HP share `event_rng_seed + floor`, aiRng uses `monster_rng_seed + floor`,
and card-random uses the reward seed plus floor. After entry, those streams have
already consumed opening shuffle, HP, and AI rolls.

The implemented prior `a0_act1_simple_combat_exchangeable_v1` does **not** claim
that generated combat RNG is that post-entry state. It independently samples
run-envelope seeds and independently samples combat stream raw states at counter
zero. That is an exchangeable-future approximation for unrealized draws.
Master-seed reconstruction and conditioning the true entry process on public
HP/intent are out of protocol for reported agents. Consumers must not treat the
sampled streams as calibrated post-entry counters.

Run-level twins (`shuffle_rng_seed` / `_counter`, `monster_rng_*`,
`card_random_rng_*`, plus potion/relic/event/merchant/treasure/misc/card
reward streams) are **forbidden** as runtime input. Occupied potion paths are
refused.

Belief RNG for particles is a **separate named sampler**. It must not be
`shuffleRng` cryptanalysis from observed draws. Hypotheses are opaque and not
publicly deserializable. Belief and rollout Debug output is redacted.

### 2.2 Pile order and identities

| Field | Class | Prior |
|---|---|---|
| Draw-pile `Vec` order without Frozen Eye / public placement | Latent-generative | Uniform permutation of the public multiset (source-backed `Collections.shuffle` is uniform; exchangeable convention makes the posterior uniform on the unknown suffix). |
| Known top prefix (Frozen Eye; later Headbutt/Scry/Warcry placements) | Public / public-history | Not sampled. V2 observation only implements Frozen Eye. |
| Discard/exhaust `Vec` order | Latent-generative | Uniform permutation of the public multiset until a mechanic reads order without shuffling. |
| `piles.limbo` | Forbidden | Queue/limbo internals. Empty at ordinary `waiting_for_player` roots. |
| `CardId` on every instance | Forbidden as input | Materializer allocates fresh deterministic IDs from public entities. |
| `ContentId` | Forbidden as input | Materializer resolves public `content_key` through authoritative content tables. |
| `MonsterId` | Forbidden as input | Materializer allocates fresh IDs from public slots. |

### 2.3 `MonsterState` private fields

| Field | Class | Notes |
|---|---|---|
| `id` | Forbidden as input | Allocate fresh from public slot. |
| `content_id` | Forbidden as input | Resolve public `content_key` through the authoritative registry. |
| `intent` under Runic Dome | Latent-generative **only** with a source-backed move table given public move history | **No table is implemented in this lane.** Escalate; do not copy the true intent from a privileged scaffold into agreement with a later reveal, and do not invent a uniform-over-enum prior. |
| `intent` when visible | Public | Already in the observation. |
| `rolled_attack_damage` before display | Latent-generative (inclusive source HP/damage ranges) until shown | After display it is implied by visible intent damage. |
| `move_history` | Public-history | Player watched executed moves; not in V2 observation. Needed for Dome posteriors. |
| `moves_executed`, `sleep_turns_remaining`, `has_siphoned`, `split_triggered`, `defensive_turns_remaining`, `mode_shift_threshold`, `initial_intent_locked`, `burns_upgraded`, `defer_awakened_one_rebirth`, `gremlin_leader_slot`, `back_attack` | Forbidden or public-history depending on UI | None are projected except `in_defensive_mode` and displayed `mode_shift`. No generative prior in this lane. |
| `powers.flight_grounding_pending`, `book_stab_count`, `spiker_thorns_buffs`, `malleable_base`, `heart_buff_count`, `invincible_max` | Forbidden | Private AI / power progress. Displayed `malleable` amount is public; `malleable_base` is not. |
| `powers.time_warp`, `beat_of_death`, `invincible`, `regeneration` | Public in the real UI when those fights occur | **Not projected.** Over-redaction for Act 3/4; out of A0 Act 1 prototype scope. |
| `vulnerable_just_applied` (monster) | Forbidden / public-history | Decrement skip; inferable if the application was observed this turn. |

### 2.4 `PlayerState` and player powers omitted from projection

| Field | Class |
|---|---|
| `energy_next_turn`, `retain_hand_next_turn`, `damage_events_this_combat` | Public-history (played Charge Battery / Equilibrium / watched hits); not projected. |
| `cannot_draw` | Public as `no_draw` power when set. |
| `no_draw_precedes_combust` | Forbidden (callback order). |
| `temp_strength` / `temp_dexterity` / `temp_thorns` / `temp_rage_block` / `no_block_turns` | Public via derived power keys. |
| `vulnerable_just_applied`, `weak_just_applied`, `frail_just_applied` | Forbidden / public-history. |
| `PlayerPowers.calm`, `divinity`, `end_turn_death`, `draw_reduction`, `draw_reduction_first_draw_seen` | Watcher / Time Eater; not in Ironclad A0 projection. Over-redaction if those powers appear. |
| `draw_trigger_power_order` | Forbidden. |

### 2.5 Relic counters not projected

`fairy_heal_percent`, `fairy_consumed`, `deferred_centennial_puzzle_draw`,
`deferred_runic_cube_draws`, `deferred_warped_tongs` are **forbidden**
(potion/action-queue internals). `necronomicon_used_this_turn` is projected
when the relic is owned; mutating it on a state without the relic is a hidden
no-op for the observation (covered by existing non-interference tests).

### 2.6 Combat scaffolding, queues, and decisions — forbidden

All `pending_*`, `defer_*`, `queued_decisions`, `card_in_use`,
`last_played_card_type`, `opening_*`, Time Warp lag flags, Nilry pause flags,
Strange Spoon defer lists, PlayTop depth/flags, bomb internals beyond the
public `bomb_*` powers, `pen_nib_double_active` during resolution,
duplication flags beyond the public `duplication` power, `mark_of_bloom`
(relic presence is public if owned), `writhing_mass_mega_debuff_triggered`,
`time_warp_end_turn*`, and `CombatDecisionState` source `CardId`s / raw pile
indices.

`CombatDecisionState` **visible** offers are public through `selection`.
Queued not-yet-active overlays are forbidden.

### 2.7 Run-level hidden (combat search adjacent)

Unentered room contents, pending card-reward generation, shop stock before
entry, relic/potion/event pool order, boss-unlock profile inputs, and
`MathUtils` process-global RNG are **forbidden** as combat-policy input.
They are out of the combat-only materializer and require a future full-run
prior.

## 3. Leak and over-redaction review

Existing Rust tests in `fair_observation.rs` already pin byte-identical
observations after draw/discard/exhaust permutation, RNG reseed, instance-id
renumbering, Runic Dome intent swaps, and private monster/relic/queue
mutation. Complementary tests require public HP, hand order, pile
membership, visible intent, Frozen Eye order, and gold to change the bytes.

**No current projection leak requiring a Lane 3 stop was found.** In
particular:

- Draw order does not leak without Frozen Eye.
- Canonical card sort is on public `FairCard` fields, not `CardId`.
- Hidden-pile selection slots are canonicalized by public card value.
- Runic Dome strips category/damage/hits.
- `fair_view` consumes no RNG (architecture requirement; existing tests cover
  non-mutation).

**Taxonomy mismatches (not treated as leaks):**

- Architecture §1 vs V1/V2: `stasis_card` is projected when present.
- `NilrysCodexCardReward` shares the Toolbox selection kind.

**Over-redaction (does not leak; can cap strength):**

- No public-history draw prefix (Headbutt, Warcry, Scry, Thinking Ahead).
- No public move-history vector (needed for Dome posteriors).
- No combat turn index (`player_turns_started` is relic-gated, not a general
  turn number — V1 contract).
- Act 3/4 monster powers (Time Warp, Beat of Death, Invincible) omitted.
- `energy_next_turn` / `retain_hand_next_turn` omitted.

The current Rust foundation does **not** add those observation fields. Runtime
public-start/event emission and validated history checkpoints remain the next
core integration gate.

## 4. Implications for fair belief search

1. The only **large, source-backed, combat-local latent** for A0 Ironclad
   `waiting_for_player` roots without Runic Dome is **unknown pile order**,
   especially the draw pile, plus **unrealized future RNG**.
2. **Runic Dome intent** is the main committed-hidden combat value. Sampling
   it requires monster move tables conditioned on public history. That prior
   is **missing**; the implementation refuses Dome roots.
3. A current observation alone cannot reconstruct history-sensitive state.
   First-decision roots require an opaque facade-issued start capability; the
   capability currently exists only in core tests. Runtime event/checkpoint
   integration is not implemented, so all mid-combat roots are currently
   refused even if a caller claims to have a complete prefix. Snapshot-only
   mid-combat roots remain forbidden.
4. The implemented Rust constructor is fresh: it allocates IDs, materializes
   supported public fields, supplies pile/RNG hypotheses, validates the state,
   and checks exact reprojection. Its current support is A0 Act 1 simple
   Strike/Defend/Bash combat at an opening Cultist root or against the
   deterministic fixture at an opening root, without selections, potions,
   powers, orbs, or stateful relics.
5. Neural inputs stay `FairCombatObservation` plus public choices/history.
   Hidden hypotheses and generated authority remain planner-internal; true
   snapshots and `full_state()` remain teacher/verifier-only.
