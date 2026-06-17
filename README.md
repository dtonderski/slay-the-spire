# Slay the Spire Ironclad Simulator

This project aims to build a headless Rust simulator for Slay the Spire, starting with the Ironclad.

The long-term goal is to support reinforcement learning agents, planning, deterministic replay, and eventually full-run evaluation. The short-term goal is much smaller: build a faithful simulator through tiny tested tasks, not a giant unverified rewrite.

## Repository Layout

The Rust simulator lives under `simulator/` so the repository can also grow RL training code, agents, experiments, and verification tooling without making the whole repo a Rust workspace.

- `simulator/`: Rust workspace for the deterministic simulator.
- `simulator/crates/sts_core/`: core simulator library.
- root docs: project roadmap, research, design, verification, and status.

## Current Scope

Only the simulator workspace skeleton exists so far. No simulator mechanics exist yet.

The first implementation milestone will be:

- Ironclad starter deck concepts
- Strike, Defend, and Bash
- one simple fixed monster
- deterministic legal actions
- deterministic transition tests
- snapshot and replay tests

## Verification Philosophy

The simulator should be deterministic from seed plus action trace. It should be verified with:

- unit tests for local mechanics
- golden tests for small transitions
- snapshot round trips
- deterministic replay
- CommunicationMod-style comparisons against the real game when parity is claimed

[CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod) and [spirecomm](https://github.com/ForgottenArbiter/spirecomm) are important reference tools for real-game state export and control.

## RL Use

Future RL agents should use the simulator through a clean environment API:

- reset
- legal action generation
- step
- snapshot/restore
- symbolic observations
- later Python bindings

Simulator mechanics must stay separate from RL feature extraction and reward shaping.

## Project Documents

- `RESEARCH.md`: prior-art and source notes
- `DESIGN.md`: architecture and risk analysis
- `VERIFICATION.md`: parity and testing strategy
- `TASKS.md`: tiny ordered implementation tasks
- `AGENT_RULES.md`: rules for future coding sessions
- `STATUS.md`: current project state
