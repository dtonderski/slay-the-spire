# Status

## What Exists

- Research and design documentation:
  - `README.md`
  - `RESEARCH.md`
  - `DESIGN.md`
  - `VERIFICATION.md`
  - `TASKS.md`
  - `AGENT_RULES.md`
  - `STATUS.md`
- Rust simulator workspace skeleton:
  - `simulator/Cargo.toml`
  - `simulator/crates/sts_core/Cargo.toml`
  - `simulator/crates/sts_core/src/lib.rs`
- Rust tooling installed via `rustup`.
- Task 0.1 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Task 0.2 added typed IDs and structured simulator errors.
- Task 0.2 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Task 0.3 added a placeholder snapshot wrapper and deterministic snapshot hash helper.
- Task 0.3 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Task 1.1 added minimal serializable combat state, card instances, combat phase, and an initial fixture.
- Task 1.1 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Task 1.2 added static starter card definitions for Strike_R, Defend_R, and Bash.
- Task 1.2 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Task 1.3 added legal combat action generation and validation for starter hand cards plus EndTurn.
- Task 1.3 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Task 1.4 added Strike transition support with energy spend, hand-to-discard movement, damage through block, and win phase detection.
- Task 1.4 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Minimal simulator implementation exists through Strike, Defend, Bash, and simplified EndTurn combat transitions.
- Task 1.5 added Defend transition support with energy spend, hand-to-discard movement, and player block gain.
- Task 1.5 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Basic ID/error, snapshot, combat state, starter card content, legal action, damage, Strike transition, and Defend transition tests exist.
- Task 1.6 added Bash transition support and minimal monster Vulnerable state.
- Task 1.6 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Bash transition and minimal Vulnerable tests exist.
- Task 1.7 added simplified EndTurn handling against a fixed monster attack, deterministic draw without shuffle, block clearing, and loss detection.
- Task 1.7 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Simplified EndTurn tests exist.
- Task 1.8 added a milestone 1 golden replay integration test and manual corpus trace.
- Task 1.8 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Milestone 1 golden replay final hash: `1de8822065abacde`.
- Read `sts_lightspeed` action/card queue notes in `RESEARCH.md` before task 2.1.
- Task 2.1 added an explicit local internal action queue and ordered event log for card transitions.
- Task 2.1 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Existing milestone 1 tests pass after queue introduction.
- Task 2.2 added structured `DamageInfo` with card source, target, and amount in damage event logs.
- Task 2.2 verification passed from `simulator/` with `stable-x86_64-pc-windows-gnu`:
  - `cargo +stable-x86_64-pc-windows-gnu fmt`
  - `cargo +stable-x86_64-pc-windows-gnu clippy`
  - `cargo +stable-x86_64-pc-windows-gnu test`
- Block and HP math is unchanged after `DamageInfo`.
- Git repository initialized for the project.

## What Is Not Implemented

- state model
- action model
- transition engine
- RNG system
- combat
- cards
- monsters
- relics
- potions
- map
- rewards
- shops
- rest sites
- events
- ascensions
- replay verifier
- RL API
- Python bindings

## Current Milestone

Milestone 2: Minimal Combat Engine.

## Next Task

Task 2.3: Add Draw and Shuffle.

## Known Risks

- Exact Slay the Spire RNG parity is the hardest part and requires controlled comparison against the real game.
- `sts_lightspeed` is strong prior art and must be studied before implementing parity-sensitive systems, but it is still a reimplementation rather than the real-game authority.
- CommunicationMod provides a practical verification route, but may not expose every hidden internal field.
- Save files may expose RNG counters needed for mid-run replay parity.
- Wiki/community references are useful for bootstrap values but cannot prove subtle ordering, RNG, or edge-case behavior.
- Scope creep is likely unless future sessions follow `TASKS.md` and `AGENT_RULES.md`.
- Starting RL training before parity milestones would risk optimizing against simulator bugs.
- The default MSVC Rust toolchain cannot currently link tests because Visual Studio C++ Build Tools failed to install with installer error `8006`; use `stable-x86_64-pc-windows-gnu` for local verification unless MSVC Build Tools are repaired later.

## Last Updated

2026-06-18.
