# Design: Ironclad Slay the Spire Simulator

## Purpose

This project is a headless Rust simulator for Slay the Spire, starting with the Ironclad. The long-term goal is to support reinforcement learning, planning, replay, and full-run evaluation. The near-term goal is much narrower: build correctness one tiny, verified mechanic at a time.

The design bias is practical fidelity. A beautiful engine that cannot be compared against the real game is a trap. Every subsystem should be deterministic, serializable, testable in isolation, and eventually checkable against traces captured from the real game.

Full fidelity is ambitious. Slay the Spire has many interacting action-queue effects, hidden RNG streams, relic hooks, event conditionals, combat-side card mutations, and save/load edge cases. The staged path is:

1. Deterministic toy combat with real Ironclad starter cards.
2. Combat parity for a small verified monster set.
3. Reward, map, shop, rest, event, relic, potion, and ascension systems.
4. Seeded-run parity using captured real-game traces.
5. Fast RL rollouts after the verified engine surface is stable.

## Research Notes

See `RESEARCH.md` for the full research pass. The short version:

- [CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod) is a Slay the Spire mod that launches an external process and exchanges JSON states/actions over stdin/stdout. Its README says it sends JSON whenever the game reaches a stable state and accepts commands such as play, end, click, wait, and state. This is the strongest known path for parity testing against the real game.
- [spirecomm](https://github.com/ForgottenArbiter/spirecomm) is a Python package and simple AI built around CommunicationMod. It is useful as an example client and schema inspiration, not as an engine design to copy.
- [sts_lightspeed](https://github.com/gamerpuppy/sts_lightspeed) is the closest known prior-art simulator: a standalone C++17 simulator/tree-search engine that claims 100% RNG-accuracy intent, broad Ironclad/enemy/relic/out-of-combat coverage, save-file loading, Java/libGDX RNG compatibility, named RNG streams, action/card queues, and Python bindings. It must be studied before implementing RNG, save loading, map/reward/shop generation, and action queue semantics.
- [silentcoder99/sts_lightspeed](https://github.com/silentcoder99/sts_lightspeed) is a fork whose GitHub description says it adds an MCTS agent and CommunicationMod integration. Inspect it before building our own real-game bridge.
- [rusted-spire](https://github.com/lhy-loveworld/rusted-spire) is a Rust headless combat simulator for RL with Strike/Defend/Bash/Jaw Worm-like early scope. It is useful as an MVP comparison, but its README says it removes the game's animation-driven action queue and defers exact Java RNG parity, which is the opposite of this project's fidelity-first long-term goal.
- [conquer-the-spire](https://github.com/utilForever/conquer-the-spire) is older C++ simulator/RL prior art.
- [bottled_ai](https://github.com/xaved88/bottled_ai), [borg_the_spire](https://github.com/elidupree/borg_the_spire), and [gym-sts](https://github.com/kronion/gym-sts) are useful examples of CommunicationMod-based bots, helpers, and real-game Gym-style environments.
- [Slay-the-Spire-data](https://github.com/MaT1g3R/Slay-the-Spire-data) and RunHistoryPlus-style tooling are useful for coarse run-history corpora, but not exact transition parity.
- [ModTheSpire](https://github.com/kiooeht/ModTheSpire) and [BaseMod](https://github.com/daviscook477/BaseMod) are the modding foundations required by CommunicationMod.
- The [Slay the Spire Wiki Ironclad card list](https://slay-the-spire.fandom.com/wiki/Ironclad_Cards) provides convenient starter values: Bash costs 2 and deals 8 plus 2 Vulnerable, Defend costs 1 and gives 5 Block, Strike costs 1 and deals 6. Treat wiki data as a bootstrap reference, not final authority.
- The [Cultist wiki page](https://slay-the-spire.fandom.com/wiki/Cultist) is a useful first monster reference: 48-54 HP at low ascension, starts with Incantation, then attacks each turn. For milestone 1, use a fixed simplified monster before adding real Cultist variance.
- A 2025 paper, [Analysis of Uncertainty in Procedural Maps in Slay the Spire](https://arxiv.org/abs/2504.03918), reports analysis over 20,000 runs. It is evidence that run-history-scale datasets exist, but not necessarily that they expose per-decision or hidden RNG state.
- A 2025 paper, [Rule Synergy Analysis using LLMs](https://arxiv.org/abs/2508.19484), is not simulator prior art, but it is relevant evidence that rule timing and state interactions are hard enough to deserve explicit tests.
- I did not find evidence, during this pass, of a maintained open-source Rust simulator that already provides full seeded Slay the Spire parity. Treat existing bot/RL projects as clients, datasets, or inspiration unless they publish exact transition-level verification.

Research follow-ups before parity claims:

- Inspect `sts_lightspeed` and its CommunicationMod-integrated fork in detail for RNG stream setup, save-file import, action ordering, map generation, reward generation, and known caveats.
- Identify exact game version and Java RNG implementation details from the target binary/decompiled references where legally available to the developer.
- Capture sample CommunicationMod states for the first real-game combat traces.
- Inventory any local RunLogger or run-history exports available to the user.
- Verify whether map, reward, monster, and shuffle RNG streams can be inferred from exported state alone or require a custom instrumentation mod.

## Version Assumptions

Primary target:

- PC Steam Slay the Spire, unmodded content, with only verification mods enabled.
- Game version must be recorded before parity work begins. Use the version exposed by the game/mod environment where possible.
- Character scope: Ironclad only.
- Ascension scope: start at A0, then add ascension deltas explicitly.
- Unlock scope: assume all Ironclad cards, relics, potions, and events are unlocked when those systems are implemented. Starter milestone ignores unlocks.

Non-goal for now:

- Slay the Spire II.
- Console/mobile ports.
- Daily climbs, custom modifiers, beta branches, speedrun timer quirks, achievements, score calculation, leaderboards.
- UI animation timing, rendering, sound, input latency.
- Modded content except verification instrumentation.

## Architecture Overview

Use a small Rust workspace under `simulator/`:

- `sts_core`: deterministic simulator state, actions, transition engine, content definitions.
- `sts_verify`: trace formats, canonical diffs, fixture loaders, real-game comparison helpers.
- `sts_rl`: optional later crate for environment wrappers and feature extraction.
- `py-sts`: later Python bindings using PyO3 or maturin.

Keep simulator logic separate from RL features. The simulator should know nothing about tensors, policies, reward shaping, or observation normalization. RL adapters consume canonical simulator state and legal actions.

## State Model

Use one authoritative state tree. Avoid deriving gameplay truth from logs or observations.

Top-level state:

- `RunState`
- `GameConfig`
- `RngState`
- `PlayerState`
- `DeckState`
- `MapState`
- `RoomState`
- `ScreenState`
- `RewardState`
- `TraceMetadata`

`GameConfig`:

- game version target
- simulator version
- character
- ascension
- seed string and decoded numeric seed
- enabled unlock set
- verification mode flags

`PlayerState`:

- current HP, max HP
- gold
- current energy, base energy, temporary energy modifiers
- block
- powers
- relics with counters and per-combat/per-turn state
- potions and potion slots
- damage counters and turn counters needed by relics/cards

`DeckState`:

- master deck outside combat
- combat draw pile
- hand
- discard pile
- exhaust pile
- limbo/play area
- card instances with stable instance IDs
- combat-only cards and generated cards
- per-card mutable state, such as cost-for-turn, cost-for-combat, misc counters, upgrade count, retain/exhaust/ethereal flags

Every card instance needs a stable simulator ID. Real game UUIDs can be captured during verification, but the simulator should not depend on matching UUID generation until that is explicitly verified.

`CombatState`:

- turn number
- phase: pre-combat, player-turn-start, waiting-for-player-action, action-queue-resolving, monster-turn, combat-complete
- player
- monsters in slot order
- action queue
- pending choices
- cards played this turn/combat
- damage instances this turn/combat
- draw/discard/exhaust counters
- temporary flags such as cannot-draw, cannot-gain-block, free-to-play once

`MonsterState`:

- monster kind
- slot index
- current/max HP
- block
- powers
- intent and move ID
- move history
- half-dead/gone/escaping flags
- per-monster RNG-relevant counters

`RunState` outside combat:

- act, floor, room path
- current room kind and phase
- map nodes and edges
- boss choice
- monster encounter history
- elite/boss/event history
- reward queues
- shop inventory
- relic pools, card pools, potion pools
- Neow bonus state

## Action Model

Use typed actions, not strings. Actions must be serializable and stable.

Core action enum:

- `StartRun { seed, character, ascension }`
- `ChooseNeowOption { option_id, choice }`
- `ChooseMapNode { node_id }`
- `PlayCard { hand_index, card_instance_id, target }`
- `UsePotion { slot, target }`
- `DiscardPotion { slot }`
- `EndTurn`
- `ChooseReward { reward_id, choice }`
- `SkipReward { reward_id }`
- `OpenChest`
- `ShopBuy { item_id }`
- `ShopRemove { card_instance_id }`
- `RestSite { action }`
- `EventChoice { choice_id }`
- `Confirm`

For RL:

- Legal actions are generated from state.
- The environment exposes an action mask and a compact action descriptor list.
- Invalid actions are rejected with errors in simulator API. RL wrappers may map invalid discrete indices to no-op only in experimental adapters, never in core.

Targets:

- `None`
- `Monster(slot)`
- `Card(instance_id)`
- `PotionSlot(slot)`
- `MapNode(id)`

Avoid relying only on hand indexes because generated choices and card movement can make indexes fragile. For compatibility with real-game traces, record both index and stable ID when possible.

## Legal Action Generation

Legal action generation is part of the simulator contract, not a UI helper.

Combat legal actions:

- `EndTurn` when waiting for player action.
- `PlayCard` for each playable hand card:
  - enough energy or free-to-play
  - target requirement satisfied
  - target alive and targetable
  - card-specific predicates satisfied
  - no blocking powers such as Entangled
- `UsePotion` for usable potions:
  - correct phase
  - target requirement satisfied
  - potion-specific restrictions satisfied
- `DiscardPotion` where game rules permit.

Screen legal actions:

- reward picks/skips
- map choices reachable from current node
- event choices currently visible
- shop purchases affordable and available
- rest-site actions allowed by relics/HP/deck state

Every legal-action function must be deterministic and side-effect free.

## Transition Engine

Core API:

- `state.legal_actions() -> Vec<ActionDescriptor>`
- `state.apply(action) -> Result<TransitionReport, SimError>`
- `state.step_internal_until_decision() -> TransitionReport`
- `state.snapshot() -> Snapshot`
- `State::from_snapshot(snapshot) -> Result<State, SnapshotError>`

`apply` should execute the external action, then resolve internal actions until the next decision point or terminal state.

Transition reports:

- previous state hash
- action
- ordered event log
- RNG draws consumed
- resulting state hash
- optional canonical diff

The event log is for debugging and verification. It must not be the source of state truth.

## Combat and Action Queue Semantics

Slay the Spire uses action-queue-like semantics. Model this explicitly from the beginning, even in milestone 1.

Recommended structure:

- `GameAction` enum for internal queued actions.
- Queue is ordered and deterministic.
- Effects enqueue follow-up actions instead of mutating everything inline when ordering can matter.
- Hooks are explicit:
  - pre-card-play
  - on-card-play
  - on-attack
  - on-damage-give
  - on-damage-receive
  - on-block-gain
  - on-card-drawn
  - on-card-exhausted
  - at-start-of-turn
  - at-end-of-turn
  - at-start-of-combat
  - at-end-of-combat

Do not build a generic event bus too early. Start with direct ordered hook calls and only extract abstractions when multiple implemented mechanics need them.

Prior-art warning: `rusted-spire` intentionally removes the original game's action queue for an RL-MVP. This project should not follow that choice. `sts_lightspeed` models both action and card queues, which better matches the full-fidelity goal.

Damage model:

- represent damage as a `DamageInfo` struct:
  - source
  - target
  - base amount
  - damage type: normal, thorns, HP loss, poison, block-loss, etc.
  - hit count
  - flags for attack, can-trigger-thorns, affected-by-strength, affected-by-vulnerable, affected-by-weak
- keep integer rounding rules explicit.

Turn flow:

1. Combat setup.
2. Start-of-combat hooks.
3. Monster intent roll.
4. Initial shuffle/draw.
5. Player turn start hooks.
6. Wait for player actions.
7. End turn:
   - discard hand as rules require
   - ethereal/exhaust handling
   - player end-turn powers
   - monster turn in slot order
   - monster end-turn powers
   - next monster intent roll
   - next player turn start

This flow must be refined against real-game traces; the document is a design starting point, not a claim of exact order.

## RNG Strategy

RNG parity is the largest technical risk.

Design requirements:

- Deterministic from seed plus action trace.
- Snapshot includes full RNG state.
- Every RNG consumption is logged with stream name, call site, input bounds, raw value if available, and resulting choice.
- RNG is never accessed through global state.

Expected game behavior to investigate, using `sts_lightspeed` as a prior-art map but the real game as authority:

- Slay the Spire uses multiple RNG streams internally, not one conceptual stream. `sts_lightspeed` models at least: AI, card-random, card reward, event, math-util, merchant, misc, monster HP, monster encounter, Neow, potion, relic, shuffle, and treasure RNG.
- Seed strings are converted to numeric seeds. Implement exact conversion only after verifying against the game.
- Java/libGDX random behavior may matter. Do not substitute Rust `rand` for parity-sensitive streams.
- Save files expose RNG seed counters that may be necessary for restoring parity mid-run.
- Some shuffles use Java `Random` seeded from Slay the Spire RNG long draws, so exact Java `Collections.shuffle` compatibility may matter.
- Some RNG consumption exists only to keep stream counters aligned. These draws must be logged and tested, not optimized away.

Implementation path:

1. Milestone 1 uses a simulator-owned deterministic RNG with logged draws, but no claim of real-game seed parity.
2. Add a `GameRng` abstraction with named streams and reproducible snapshots.
3. Implement exact libGDX/Slay-the-Spire RNG and Java shuffle compatibility behind tests.
4. Build tiny tests for each discovered random decision.
5. Compare generated draws/choices against save-file counters, CommunicationMod traces, and optionally `sts_lightspeed` as a differential oracle.
6. Replace placeholder RNG algorithms with exact game-compatible implementations only when evidence is available.

RNG API shape:

- `rng.next_int(stream, bound, callsite)`.
- `rng.next_float(stream, callsite)`.
- `rng.shuffle(stream, slice, callsite)`.
- `rng.choice(stream, weighted_table, callsite)`.

Never call RNG from display, serialization, legal-action generation, hashing, or RL feature extraction.

## Content Representation

Use data plus Rust behavior, not a giant speculative scripting engine.

Cards:

- `CardDef` contains static data: ID, name, color, rarity, type, target mode, base cost, upgrade metadata, keywords.
- `CardBehavior` is implemented in Rust per card or per simple reusable effect.
- Start with Strike, Defend, Bash.
- Add cards in small groups only when tests cover each card's legal play, transition, upgrade, and interactions.

Relics:

- `RelicDef` plus Rust behavior hooks.
- Include counters/state in `RelicInstance`.
- Burning Blood should wait until combat-end healing and max-HP boundaries are testable.

Potions:

- `PotionDef` plus behavior.
- Potions need target rules, combat/out-of-combat usability, slot rules, discard rules, and reward/shop generation.

Powers:

- Powers are stateful modifiers with amount, optional secondary amount, source, and owner.
- Implement powers as explicit Rust hook handlers.
- Vulnerable, Weak, Strength, Dexterity, Frail, Ritual, Metallicize, Barricade-style retention, and temporary strength loss should be added incrementally.

Monsters:

- `MonsterDef` has HP range by ascension, move table, intent display, and behavior.
- Move selection is code, not static tables only, because many monsters have history rules.
- First simple monster should be fixed and deterministic.
- First real monster candidate: Cultist, because its pattern is simple.

Events:

- Data for text/choice IDs can be separate, but behavior should be Rust functions until there is enough repetition to justify DSL-like helpers.
- Event availability and outcome RNG need trace-based parity.

## Run Systems

Map:

- Nodes with act, row, x, room kind, children, parents.
- Map generation must eventually match game seeds.
- Until parity is researched, allow fixed maps for early combat/reward testing.

Rewards:

- Reward queue with typed reward items: gold, card choice, relic, potion, emerald/sapphire/ruby keys later.
- Card reward generation depends on pools, rarity roll, duplicate rules, character, unlocks, and special screens.
- Skipping is an explicit action.

Shop:

- Shop inventory snapshot is fixed once generated.
- Include card, relic, potion, removal price, sale flags, and membership in pools.
- Shop generation parity comes after basic reward parity.

Rest sites:

- Rest, smith, and later lift/dig/recall/toke-like actions.
- Legal actions depend on relics, HP, deck, keys, and character content.

Events:

- Current event screen is a decision point.
- Choices must be stable IDs.
- Effects should produce normal transition reports and RNG logs.

Ascensions:

- Treat ascension as layered rule modifiers.
- Do not sprinkle `if ascension >= N` everywhere without tests.
- Add ascension gates only when the underlying mechanic exists.

## Serialization and Snapshots

Use `serde` for human-readable JSON snapshots first. Consider binary snapshots only after performance evidence demands it.

Snapshot requirements:

- complete enough to resume exactly
- stable schema version
- canonical ordering
- no derived/cache-only fields unless marked
- content version and simulator version included
- RNG state included
- action trace ID included

State hashing:

- canonical JSON or a custom canonical encoder
- exclude debug-only fields and non-authoritative logs
- include all gameplay-affecting state

Snapshot classes:

- `FullSnapshot`: exact resume.
- `ObservationSnapshot`: what an agent or CommunicationMod-like observer can see.
- `DebugSnapshot`: full state plus logs, diffs, RNG details.

## RL API

The Rust core should expose a simple environment API:

- `reset(config) -> Observation`
- `legal_actions() -> Vec<ActionDescriptor>`
- `step(action_index_or_action) -> StepResult`
- `snapshot() -> bytes/json`
- `restore(snapshot)`
- `seed_info()`

`StepResult`:

- observation
- legal action mask
- reward
- done
- terminal reason
- info containing state hash, floor, combat outcome, and optional debug diff

Reward shaping belongs outside the simulator. The core can expose signals such as HP delta, floor, victory, death, card picked, boss defeated, but should not decide RL reward policy.

Observation layers:

- exact symbolic state for planning
- masked/visible state approximating real player information
- later tensor-friendly feature extraction in a separate crate

## Python Binding Plan

Use PyO3/maturin after the Rust API stabilizes.

Bindings should expose:

- `Env`
- `Snapshot`
- `Action`
- `ActionDescriptor`
- batch reset/step later

Python should not reimplement mechanics. It should call the Rust core and handle training libraries.

For high-throughput RL:

- support many environments in one Rust object
- avoid JSON in hot loops
- keep JSON snapshots for debug and test artifacts
- optionally expose NumPy arrays for observations once feature formats are stable

## Performance Strategy

Correctness first, then measured optimization.

Initial choices:

- small enums and integer IDs for content
- Vec-backed piles and hands
- no dynamic dispatch in hot combat paths unless profiling supports it
- cloneable state for planning, but watch allocation behavior
- deterministic state hashes only when requested or in debug/test mode

Later optimizations:

- arena or slot-map for card instances if instance churn becomes expensive
- compact binary snapshots
- batched rollouts
- feature extraction caches outside authoritative state
- disable event logs in release rollout mode

Do not optimize away traceability before parity is established.

## Biggest Risks and Mitigations

RNG parity:

- Risk: seed plus action trace diverges because stream order is wrong.
- Mitigation: named RNG streams, draw logs, CommunicationMod trace comparison, isolated RNG tests.

Action queue ordering:

- Risk: card/relic/power hooks fire in subtly wrong order.
- Mitigation: event logs, tiny golden tests, parity traces around one interaction at a time.

Hidden game state:

- Risk: real game contains counters or pools not visible in exported states.
- Mitigation: capture richer mod traces where possible; otherwise infer using controlled experiments and mark hidden fields explicitly.

Scope explosion:

- Risk: vibe-coded sessions add broad mechanics without verification.
- Mitigation: `TASKS.md`, `AGENT_RULES.md`, `STATUS.md`, and strict one-task sessions.

Content volume:

- Risk: hundreds of cards/relics/events create unreviewable code.
- Mitigation: implement content in dependency order, with per-content tests and no giant content dumps.

RL pressure:

- Risk: training starts before simulator is faithful enough, baking bugs into policies.
- Mitigation: separate RL adapters, publish fidelity level, run parity gates before evaluation claims.
