# Slay the Spire Simulator and RL Workspace

This repository is organized into two source areas with a one-way dependency:
**`rl` may depend on `simulator`; `simulator` must not depend on `rl`.**

## Areas

- [`simulator/`](simulator/README.md): Rust simulator, verifier, Python binding,
  trace corpus, collection tools, and support mods.
- [`rl/`](rl/README.md): synthetic combat training, evaluation, sampling tools,
  and the simulator combat explorer.
- [Project knowledge](docs/README.md): navigation to canonical contracts,
  workflows, durable findings, research proposals, and project history.
- [`PROJECT_OVERVIEW.md`](PROJECT_OVERVIEW.md): objective and fair-state boundary.
- [`AGENTS.md`](AGENTS.md): development and verification rules.

The root [`Cargo.toml`](Cargo.toml) and `Cargo.lock` coordinate the Rust
workspace. See each area README for commands and detailed documentation.
