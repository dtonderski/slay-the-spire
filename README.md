# Slay the Spire Ironclad Simulator

This project aims to build a headless Rust simulator for Slay the Spire, starting with the Ironclad.

The long-term goal is to support reinforcement learning agents, planning, deterministic replay, and eventually full-run evaluation. The short-term goal is much smaller: build a faithful simulator through tiny tested tasks, not a giant unverified rewrite.

## Repository Layout

The Rust simulator lives under `simulator/`, together with its verification corpus.

- `simulator/`: Rust workspace for the deterministic simulator.
- `simulator/crates/sts_core/`: core simulator library.
- `simulator/crates/sts_live/`: live trace collection backend, CLI, HTTP API, and Stage 1 web shell.
- `simulator/verification/`: captured traces and permanent parity corpus.
- `docs/`: project design, history, research, and literature review.
- `tools/communication/`: required CommunicationMod client bridge and diagnostics.
- `mods/`: small project-specific game mods.

## Current Scope

The active repository is centered on the Rust simulator, live collector, and
verification corpus.

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

- `docs/research.md`: prior-art and source notes
- `docs/design.md`: architecture and risk analysis
- `PROJECT_OVERVIEW.md`: high-level RL roadmap, phase gates, evaluation protocol, and state-visibility boundaries
- `simulator/docs/verification.md`: parity and testing strategy
- `simulator/docs/live_trace_ui_design.md`: live trace collection UI design
- `AGENT_RULES.md`: rules for future coding sessions
