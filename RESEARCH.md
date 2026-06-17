# Research Notes

This file records prior art and evidence gathered before implementation. It is intentionally separate from `DESIGN.md` so future coding sessions can revisit sources without bloating the architecture document.

## Key Prior Art

### gamerpuppy/sts_lightspeed

Repository: <https://github.com/gamerpuppy/sts_lightspeed>

Why it matters:

- Closest known prior art for this project.
- C++17 standalone Slay the Spire simulator and tree-search engine.
- README claims it is designed to be "100% RNG accurate".
- README reports speed of about 1M random playouts in 5s with 16 threads.
- README claims implementation progress includes all enemies, all relics, all Ironclad cards, all colorless cards, and everything outside combat/all acts.
- Supports loading from save files, with README caveat that loading into combat was currently the supported path.
- Exposes Python bindings through `pybind11`.

Architecture observations from source inspection:

- Has explicit `GameContext` and `BattleContext` split.
- Uses many named RNG streams: `aiRng`, `cardRandomRng`, `cardRng`, `eventRng`, `mathUtilRng`, `merchantRng`, `miscRng`, `monsterHpRng`, `monsterRng`, `neowRng`, `potionRng`, `relicRng`, `shuffleRng`, `treasureRng`.
- Implements Slay the Spire/libGDX-style xorshift RNG and Java `Random`/`Collections.shuffle` compatibility.
- Save loading restores RNG streams from seed counters such as `potion_seed_count`, `relic_seed_count`, `event_seed_count`, `monster_seed_count`, `merchant_seed_count`, `card_random_seed_count`, `card_seed_count`, and `treasure_seed_count`.
- Combat uses explicit `ActionQueue` and `CardQueue`, with `addToTop`/`addToBot` ordering. This confirms action queue fidelity is not optional for full parity.
- Code comments include uncertainty and game-specific quirks, for example time-based `mathUtilRng`, combat-victory queue clearing, and actions that consume RNG only to keep parity.

Design lessons:

- We should not pretend to be first. `sts_lightspeed` should be studied carefully before implementing RNG, save loading, map generation, reward generation, and action queues.
- We should not blindly port it. The Rust project needs stronger snapshot/replay tests, canonical diffs, and a smaller task discipline because the user wants safe vibe-coded implementation.
- The RNG stream list in `DESIGN.md` should be upgraded from speculative examples to a known reference list, while still requiring verification against the target game version.
- A future task should create `docs/prior_art/sts_lightspeed.md` or equivalent with exact behavior notes before implementing parity-sensitive systems.

### silentcoder99/sts_lightspeed

Repository: <https://github.com/silentcoder99/sts_lightspeed>

Why it matters:

- GitHub description says it is a fork of gamerpuppy's headless implementation plus MCTS agent with CommunicationMod integration.
- This may be useful for differential verification, especially if it contains practical bridges between real-game state and the simulator.

Design lessons:

- Before writing our own CommunicationMod verifier, inspect this fork for integration patterns.
- Do not assume the fork is more correct than upstream; use it to identify useful trace/control workflows.

### lhy-loveworld/rusted-spire

Repository: <https://github.com/lhy-loveworld/rusted-spire>

Why it matters:

- Rust headless Slay the Spire combat simulator for reinforcement learning.
- As of the inspected README/PLAN, it implements a scope similar to our proposed first combat milestone: Strike, Defend, Bash, Jaw Worm, combat loop, damage pipeline, named RNG streams, and planned PyO3 bindings.
- It explicitly says exact pixel-perfect RNG match with the Java game is deferred/out of scope for now.
- Its README says it intentionally removes the original game's animation-driven action queue and executes effects immediately.

Design lessons:

- Useful cautionary comparison: good for RL-MVP ergonomics, not sufficient as a fidelity model.
- Our design should differ by keeping action queue semantics from the beginning, even if milestone 1 uses only a tiny subset.
- The task plan should mention `rusted-spire` as an existence proof for the minimal Rust combat MVP, but should not copy its "no action queue" choice because this project prioritizes future parity.

### utilForever/conquer-the-spire

Repository: <https://github.com/utilForever/conquer-the-spire>

Why it matters:

- C++ Slay the Spire simulator with reinforcement-learning ambitions.
- Older project, last pushed in 2020 during the inspected metadata.
- Useful mostly as historical RL/simulator prior art, not as a direct parity reference unless its internals prove otherwise.

Design lessons:

- Simulator/RL coupling can age poorly. Keep core simulator, verification, and RL wrappers separate.

### ForgottenArbiter/CommunicationMod

Repository: <https://github.com/ForgottenArbiter/CommunicationMod>

Why it matters:

- Slay the Spire mod that launches an external process and communicates over stdin/stdout.
- Sends JSON state whenever the game reaches a stable state.
- Accepts commands such as `play`, `end`, `key`, `click`, `wait`, and `state`.
- Requires ModTheSpire and BaseMod.

Design lessons:

- Primary real-game parity harness.
- Its state is observable game state, not necessarily full hidden state or exact RNG stream positions.
- The simulator snapshot schema should be designed to normalize CommunicationMod JSON while still retaining hidden simulator fields.

### ForgottenArbiter/spirecomm

Repository: <https://github.com/ForgottenArbiter/spirecomm>

Why it matters:

- Python package for interfacing with CommunicationMod plus a simple AI.
- Useful for protocol/client patterns.

Design lessons:

- Good source for action/state schema examples.
- Not a simulator architecture model.

### xaved88/bottled_ai

Repository: <https://github.com/xaved88/bottled_ai>

Why it matters:

- Actively developed Python bot for Slay the Spire using manually constructed strategies.
- README describes combat search over possible hand play orders with a custom simulation/evaluation.
- README states it does not access secret information such as future random rolls or draws.

Design lessons:

- Useful for separating "planner using visible state" from "simulator with hidden state".
- RL/planning APIs should support visible-observation mode, not just omniscient simulator state.

### elidupree/borg_the_spire

Repository: <https://github.com/elidupree/borg_the_spire>

Why it matters:

- Rust helper/AI built on CommunicationMod.
- Splits live communication from CPU-heavy analysis through a saved state file and browser UI.

Design lessons:

- Useful pattern for verifier tooling: capture real-game states to files, then run heavy diff/analysis separately.

### kronion/gym-sts

Repository: <https://github.com/kronion/gym-sts>

Why it matters:

- OpenAI Gym environment that runs the real game with ModTheSpire, BaseMod, CommunicationMod, and SuperFastMode.
- Can run the game headless in Docker.

Design lessons:

- Useful baseline for real-game-backed RL or verification.
- Too slow/heavy for many simulator rollouts, but valuable for trace generation and parity smoke tests.

### MaT1g3R/Slay-the-Spire-data

Repository: <https://github.com/MaT1g3R/Slay-the-Spire-data>

Why it matters:

- Run-history datasets for streamers, including an Ironclad sample.
- README says it can analyze local run-history folders and recommends Run History Plus.

Design lessons:

- Good for distribution checks, deck/path/reward outcome corpora, and high-level regression examples.
- Not enough for exact transition parity because run histories generally lack per-action hand/draw/discard/action-queue/RNG data.

### modargo/RunHistoryPlus

Repository: <https://github.com/modargo/RunHistoryPlus>

Why it matters:

- Slay the Spire mod for richer run histories.
- Used or recommended by run-history analysis tooling.

Design lessons:

- Investigate for coarse corpus generation.
- Do not treat it as a substitute for CommunicationMod traces.

## Datasets and Papers

### Analysis of Uncertainty in Procedural Maps in Slay the Spire

Paper: <https://arxiv.org/abs/2504.03918>

Why it matters:

- Uses a dataset of 20,000 Slay the Spire runs to analyze path uncertainty and outcomes.
- Confirms that run-history-scale data exists and can support distribution-level analysis.

Design lessons:

- Useful for high-level evaluation later.
- Does not solve transition-level simulator verification.

### Rule Synergy Analysis using LLMs

Paper: <https://arxiv.org/abs/2508.19484>

Why it matters:

- Uses Slay the Spire card synergy/rule interactions as a benchmark domain.
- Reports that models struggle with timing, state definition, and rule interactions.

Design lessons:

- Reinforces that action ordering and explicit state modeling are core risks.
- Future vibe-coded mechanics should require design notes and interaction tests.

## Practical Conclusions

1. `sts_lightspeed` is mandatory prior art for RNG, action queue, save loading, and content coverage.
2. A Rust rewrite is still justified if the goal is maintainable incremental correctness, strong tests, deterministic replay artifacts, and clean RL bindings.
3. Full game fidelity should be staged and evidence-driven. Existing projects make it plausible, not free.
4. CommunicationMod remains the real-game authority for observed-state parity.
5. Save files may expose RNG counters that CommunicationMod states do not. Save-file import should become an earlier verification tool than originally planned.
6. The simulator should support both omniscient state for replay/debug and visible state for agents that should not exploit hidden information.
7. Immediate-effect combat engines are attractive for RL speed but are the wrong default for long-term Slay the Spire parity.

## Save-File/RNG Research Gate

Task 2.4 finding:

- The existing 0.0 notes identify these real save-file RNG counter fields as parity-relevant: `potion_seed_count`, `relic_seed_count`, `event_seed_count`, `monster_seed_count`, `merchant_seed_count`, `card_random_seed_count`, `card_seed_count`, and `treasure_seed_count`.
- These fields matter because mid-run replay needs per-stream advancement counters, not just base seeds.
- The 0.0 `sts_lightspeed` notes say its save loading restores RNG streams from those same seed counters, matching the field list above.
- The public `gamerpuppy/sts_lightspeed` README describes the project as RNG-accurate, save-file loading capable, and able to search while knowing the game's RNG state. That supports treating its save/RNG handling as high-priority prior art, but it is not source-level proof of the exact field mapping.
- Decision: save import should move earlier than broad map/reward/shop RNG parity work, but after local snapshot/replay and RNG stream structure are stable.
- Gate before implementation: inspect exact `sts_lightspeed` save-loading source files and real decrypted save examples. Record source file/function names and confirm whether each listed counter maps to a named simulator RNG stream.

Current limitation:

- No full save importer, decryption tooling, or broad RNG parity claim exists in this repo.
