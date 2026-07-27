# Simulator

Rust workspace for the deterministic Slay the Spire simulator, live collector,
combat agent, Python bindings, and strict real-game verifier.

## Layout

- `crates/sts_core/`: simulator mechanics and deterministic run state.
- `crates/sts_verify/`: strict seed-plus-actions trace replay.
- `crates/sts_live/`: CommunicationMod bridge backend, CLI, UI, and combat agent.
- `crates/py_sts/`: optional PyO3 bindings.
- `verification/corpus/`: captured traces and the permanent regression corpus.
- `docs/`: simulator design and status notes.

## Verification

From this directory:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run -- cargo run -p sts_verify -- parity verification\corpus\communication_mod\<trace>.jsonl
```

The verifier always derives simulator state from the recorded `START` seed and
subsequent actions. Real-game observations are comparison evidence, never
simulator-state hydration.

To export the authoritative simulator endpoint from a trace, use the explicit
replay command:

```powershell
cargo run -p sts_verify -- replay --json verification\corpus\permanent_traces\trace-session-8.jsonl -o replay.json
cargo run -p sts_verify -- replay --json --at-step 3322 verification\corpus\permanent_traces\trace-session-8.jsonl
cargo run -p sts_verify -- replay --timeline verification\corpus\permanent_traces\trace-session-8.jsonl
```

Replay returns the final snapshot, a state-hash checkpoint for each trace
action, and an optional snapshot at or before `--at-step`. It exits `0` when
the trace reaches its endpoint, `1` for invalid input, and `2` when the
authoritative simulator reaches a replay boundary. Observed game state never
changes the exported snapshot.

## Live CLI

CommunicationMod launches `..\tools\communication\trace_client.js`. With the
game bridge running, use the Linux CLI from WSL:

```bash
cd /mnt/d/dev/slay-the-spire/simulator
export STS_LIVE_BRIDGE_SESSION_DIR=/mnt/d/dev/slay-the-spire/tools/communication/session
cargo run -p sts_live --bin live-trace -- bridges list
cargo run -p sts_live --bin live-trace -- sessions list
```

If CommunicationMod was launched from another worktree, point
`STS_LIVE_BRIDGE_SESSION_DIR` at that worktree's shared session directory.
Keep `STS_LIVE_ALLOW_FILE_COMMANDS` unset: normal play uses guarded TCP control.

Replay a verified CommunicationMod or `sts_live` JSONL trace into the real game
with the same Linux CLI:

```bash
cargo run -p sts_live --bin live-trace -- replay /path/to/source.jsonl --dry-run
cargo run -p sts_live --bin live-trace -- replay /path/to/source.jsonl \
  --bridge communication-mod --reset-bridge
```

Replay validates the source against the simulator before touching the game. It
starts the recorded character, ascension, and seed, requires any captured
profile input to match the live profile, then matches each recorded command to
one current enabled legal action. It stops before an unavailable command or as
soon as live fidelity is not `ok`. Without `--reset-bridge`, an active run is
never abandoned. Use `--max-actions N` to replay only a verified prefix after
`START`; `--dry-run` performs no bridge operations. Normal live replay does not
accept `START_VERIFY` traces or traces with explicit boss-unlock inputs that the
live bridge cannot assert.
