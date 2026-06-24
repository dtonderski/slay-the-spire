# sts_lightspeed Comparison Audit

This document compares `gamerpuppy/sts_lightspeed` against the current local simulator before any RNG refactor. It is a research artifact only: no gameplay behavior changes are implied by this document.

Sources inspected:
- `gamerpuppy/sts_lightspeed` README at `https://github.com/gamerpuppy/sts_lightspeed`
- external checkout at `gamerpuppy/sts_lightspeed` `master`, inspected locally in a temporary clone
- local simulator sources under `simulator/crates/sts_core/src`
- local verifier sources under `simulator/crates/sts_verify/src`

The upstream README describes `sts_lightspeed` as a standalone C++17 simulator designed for RNG accuracy, with save-file loading, tree search, all enemies, all relics, all Ironclad cards, all colorless cards, and outside-combat/all-act coverage. Treat those claims as scope signals, not final authority; the real game and local CommunicationMod traces remain the verification authority.

## Matrix

| sts_lightspeed concept | our equivalent | observed gap | proposed local change | confidence | verification needed |
| --- | --- | --- | --- | --- | --- |
| `Random` in `include/game/Random.h`: libGDX-style `RandomXS128` wrapper with public draw counter. | `StsRng` in `sts_core/src/rng.rs`, including `with_counter`, `counter`, integer/float/bool/long draws. | The local implementation has the right shape and tests, but stream state is not centralized; counters are copied into many `RunState` fields and verifier reconstruction sites. | Keep `StsRng`; wrap seed/counter pairs in a typed run stream-state abstraction after this audit. | High | Golden tests for `random_int`, `random_int_range`, `random_float`, `random_long`, and `with_counter` against bytecode-derived or save-derived fixtures. |
| Java `Random` and `Collections.shuffle` compatibility in `include/game/Random.h`. | `JavaRng` and `StsRng::collections_shuffle` in `sts_core/src/rng.rs`; relic pool initialization uses Java-compatible shuffle paths. | Local naming conflates two shuffle compatibility cases: Java LCG shuffles and raw `RandomXS128` collection-style shuffles. | Document each shuffle caller with the RNG family used by the target game; avoid adding a single generic `shuffle` API until callers are classified. | Medium | Fixtures for relic pool initialization, deck shuffle, event/relic list shuffles, and face event shuffle behavior. |
| Named `GameContext` RNG streams: `aiRng`, `cardRandomRng`, `cardRng`, `eventRng`, `mathUtilRng`, `merchantRng`, `miscRng`, `monsterHpRng`, `monsterRng`, `neowRng`, `potionRng`, `relicRng`, `shuffleRng`, `treasureRng`. | Local `RunState` tracks `event_rng_seed`, `reward_rng_seed`, `card_rng_counter`, `card_random_rng_counter`, `treasure_rng_seed/counter`, `potion_rng_seed/counter`, `relic_rng_seed/counter`, `merchant_rng_seed/counter`, `misc_rng_seed/counter`; combat tracks `shuffle_rng` and `card_random_rng`. | Several upstream streams have no first-class local run field yet: `aiRng`, `monsterHpRng`, `monsterRng`, `neowRng`, and `mathUtilRng`. `reward_rng_seed` currently doubles as the card reward seed. | Introduce `RunRngStreams` with typed stream names, but preserve serde compatibility for existing flat fields. Add explicit aliases/mappings for missing streams before changing draw order. | High | Before/after JSON compatibility tests for `RunState`; seed-start trace reports must remain unchanged. |
| Save-file counter fields in `include/game/SaveFile.h`: `potion_seed_count`, `relic_seed_count`, `event_seed_count`, `monster_seed_count`, `merchant_seed_count`, `card_random_seed_count`, `card_seed_count`, `treasure_seed_count`, plus `card_random_seed_randomizer`. | `RunState` has direct seed/counter fields; `VERIFICATION.md` already lists these save counters as future mapping work. | Local state does not yet model save import as a first-class restore path. Missing mapping for `monster_seed_count` and randomizer semantics. | Add a save-counter mapping table before implementing save import. Do not collapse fields until each save counter has a local destination and restore-then-draw test. | High | Real save samples plus CommunicationMod next-state checks; one fixture per stream proving restore-then-next-draw parity. |
| `GameContext::initFromSave` and `Deck::initFromSaveFile`. | No general save import path; verifier reconstructs `RunState` from observed CommunicationMod state in `sts_verify/src/sim_real.rs`. | Observed-state reconstruction is useful for verification, but it cannot prove hidden RNG/pool parity from save state. | Treat save import as a verifier feature: build it after stream centralization and before broad seeded-run parity expansion. | High | Load a real save, run one next legal action in game and simulator, compare supported state plus counters. |
| Combat `ActionQueue` with `addToTop`, `addToBot`, and pop-front execution. | `InternalAction` queue in `combat/transition.rs`; `process_internal_queue` pops front, appends follow-ups, and records event log. | Local queue is simpler and still lacks many target action subtleties. It now has a clean boundary after `card_effects.rs` extraction. | Keep `transition.rs` as executor; add action-order tests when implementing new relic/power hooks instead of broad rewriting. | Medium | Differential tests for known order-sensitive interactions: Dead Branch, Dark Embrace, Feel No Pain, Gremlin Horn, Unceasing Top, Time Eater, end-turn queues. |
| Combat `CardQueue` with `CardQueueItem`, card target, energy-on-use, purge/exhaust flags, and end-turn item. | Local card play queues are represented as immediate `InternalAction` sequences built in `combat/card_effects.rs`; end turn is separate `CombatAction::EndTurn`. | Local model has no separate card queue item abstraction. That is acceptable for current scope but may be insufficient for Time Eater, Havoc, duplication, purge, and queued end-turn semantics. | Do not add a full card queue yet. Add it only when a verified target behavior requires card queue identity beyond `InternalAction` order. | Medium | Targeted traces or fixtures for Havoc, Distilled Chaos, Time Eater, duplication potion, Necronomicon, and end-turn card queue behavior. |
| Card behavior organization: card-specific logic in `BattleContext`, `CardInstance`, and action helpers. | Card queue construction in `combat/card_effects.rs`; execution in `combat/transition.rs`; static definitions in `content/cards.rs`. | Local split is now more reviewable, but metadata and behavior remain duplicated across definitions, legality, rewards, and verifier key mapping. | After RNG audit work, centralize card metadata queries without moving behavior into a DSL. | High | Compile-time tests for definition lookup, upgrade mapping, reward rarity/type, starter/basic status, and verifier key names. |
| Monster AI uses `aiRng`; monster HP uses `monsterHpRng`; monster groups also use `miscRng` for composition details. | Local monster behavior is split across `content/monsters.rs`, `combat/turn.rs`, `combat/damage.rs`, and seed-start verifier helpers. Encounter generation is partially in `content/encounters.rs`. | Local state does not expose `aiRng` or `monsterHpRng` as run streams. Some verifier paths use source-backed predictions and observed sync boundaries. | Add explicit monster stream mapping before changing combat entry RNG. Keep observed-sync boundaries documented until stream parity is proven. | Medium | Captured combat-entry fixtures checking encounter choice, monster HP, opening intent, and first post-turn AI roll. |
| Encounter pools and `GameContext::populateMonsterList`. | `content/encounters.rs`, `sts_verify/src/m22.rs`, and seed-start trace checks. | Local encounter parity is source-backed for several Act 1 prefixes but not generalized across all acts/rooms. | Keep encounter generation independent from RNG refactor except for stream-state plumbing. | Medium | Act 1 multi-seed corpus plus later Act 2/3/4 encounter lists and boss selection. |
| Map generation in `src/game/Map.cpp`, with path generation, room assignment, and burning elite assignment. | `map/generation.rs`, `map/target.rs`, `run/map.rs`, and verifier map subset checks. | Local map parity is bounded to captured Act 1 traces; topology and room generation are not yet a general save/load compatible surface. | Do not mix map algorithm work into RNG stream refactor. Stream centralization should only preserve seeds/counters used by current map tests. | Medium | Map topology fixtures for known seeds, room assignment fixtures, burning elite fixtures, and path choice replay. |
| Combat rewards in `GameContext::createCombatReward`, card reward creation, potion/relic/gold helpers. | `run/reward.rs`, `content/reward_pool.rs`, `content/shop_pool.rs`, seed-start reward verifier paths. | Local reward parity is relatively advanced for captured paths, but still has explicit unobservable fields and some deferred UI timing assumptions. | Use reward code as the first consumer when replacing flat `card_rng_counter`, `potion_rng_counter`, `relic_rng_counter`, and `treasure_rng_counter`. | High | Existing `cargo test`; before/after seed-start report comparison on VERIFY01, CODEX03, CODEX04, TEST traces. |
| Shop generation in `src/game/Shop.cpp`: class cards from `cardRng`, prices and sale/relic tiers from `merchantRng`, potions from `potionRng`, relics from relic pools. | `run/shop.rs`, `content/shop_pool.rs`, verifier TEST shop trace assertions. | Local shop parity is source-backed for the captured TEST path, but stream ownership is spread across `RunState` fields. | Add stream accessors before refactoring shop internals; avoid changing inventory draw order. | High | TEST shop trace, inventory labels, prices, sale slot, purge price, restock behavior. |
| Relic pools in `RelicContainer`/`GameContext`, save mappings, and Java shuffle use. | `relic/mod.rs`, `run/state.rs`, `run/reward.rs`, `run/shop.rs`; `relic_pools` and `relic_keys` in `RunState`. | Local relic model is partial but already has pool state and source-backed reward/shop use. Stream centralization must preserve pool initialization side effects. | Keep relic pool state separate from RNG stream state; test pool initialization counter advancement explicitly. | High | Existing relic pool tests plus save-restore test once save import exists. |
| Potion generation from class pool, rarity rolls, limited flags, and potion chance. | `potion/mod.rs`, `run/reward.rs`, `run/shop.rs`; `potion_chance`, `potion_rng_seed/counter` in `RunState`. | Local potion effects and reward/shop generation are split; some potion combat temporary-state behavior remains an observed-sync boundary. | RNG stream refactor should not modify potion behavior. Add typed `potion` stream accessor first. | High | Reward potion chance tests, shop potion tests, Power Potion/temporary card traces when implemented. |
| Neow option and reward generation in `src/game/Neow.cpp`, using `neowRng` and sometimes `cardRng`. | Seed-start verifier has captured-branch Neow helpers in `sim_real.rs`; core run state does not yet have a Neow phase model. | Neow is verifier-scaffolded rather than core-modeled. No first-class `neow_rng` field exists. | Add `neow_rng` to the stream mapping design before centralization, but do not require full core Neow implementation for the RNG refactor. | Medium | Captured START traces for VERIFY01/CODEX03/CODEX04/M seeds; option lists and branch rewards. |
| Event generation via `eventRng`, `miscRng`, and event lists. | `run/event.rs`; verifier has captured TEST event-room paths. | Local event selection is partial and some hidden event state is pinned by trace assumptions. | Keep `event_rng` and `misc_rng` distinct in the typed stream abstraction. | Medium | Event-room trace replay and event-specific RNG fixtures. |
| Search/tree-search API uses known RNG state as a strength. | Local project is simulator/verifier-first; no tree search core dependency. | No gap for current refactor. Tree search should stay outside core simulator. | No local change. | High | None for RNG refactor. |

## Stream Mapping Draft

| Target/sts_lightspeed stream | Current local field or location | Status |
| --- | --- | --- |
| `aiRng` | none first-class; monster AI state in combat/verifier | Missing |
| `cardRandomRng` | `RunState.card_random_rng_counter`; `CombatState.card_random_rng`; seed derived from `reward_rng_seed + current_floor` in `RunState::card_random_rng` | Partial |
| `cardRng` | `RunState.reward_rng_seed` plus `RunState.card_rng_counter` | Present but poorly named |
| `eventRng` | `RunState.event_rng_seed`, `RunState.event_rng_counter` | Present |
| `mathUtilRng` | none first-class | Missing |
| `merchantRng` | `RunState.merchant_rng_seed`, `RunState.merchant_rng_counter` | Present |
| `miscRng` | `RunState.misc_rng_seed`, `RunState.misc_rng_counter` | Present |
| `monsterHpRng` | none first-class; generated/verified indirectly | Missing |
| `monsterRng` | none first-class; encounter generation helpers use seed-derived state | Missing |
| `neowRng` | verifier branch helpers only | Missing |
| `potionRng` | `RunState.potion_rng_seed`, `RunState.potion_rng_counter` | Present |
| `relicRng` | `RunState.relic_rng_seed`, `RunState.relic_rng_counter` | Present |
| `shuffleRng` | `CombatState.shuffle_rng` | Present in combat, not as run stream |
| `treasureRng` | `RunState.treasure_rng_seed`, `RunState.treasure_rng_counter` | Present |

## Recommended Next Local Change

The next implementation refactor should be a compatibility-preserving RNG stream wrapper:

1. Add a small `RunRngStreamState { seed: u64, counter: u32 }`.
2. Add typed accessors on `RunState` for existing streams without removing the flat serialized fields.
3. Convert reward/shop/event/potion code to use the accessors one module at a time.
4. Only after all callers use accessors, consider a custom serde migration from flat fields to a grouped stream struct.

Do not change draw order, field names, or serialization shape in the first RNG stream patch.

## Verification Gate For RNG Refactor

Before merging a central RNG stream refactor, run:

```text
cargo test
```

Then compare before/after seed-start verifier reports on representative CommunicationMod traces:

```text
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T16-45-23-530Z.jsonl
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-21T09-57-10-380Z.jsonl
```

Expected result: no new unexpected diffs, no changed first boundary for traces that intentionally stop at known unsupported scope, and unchanged RNG boundary descriptions unless the change explicitly updates documentation.
