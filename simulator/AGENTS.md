# Simulator Agent Rules

These rules apply to all work under `simulator/`.

## No Seed-Specific Behavior

Do not add seed-specific branches, allowlists, fixtures, overrides, or trace
identity tables to simulator or verifier implementation code in order to make a
trace pass.

This is forbidden in core simulator code, verifier/replay code, UI bridge code,
and Python orchestration code. Examples of unacceptable patterns:

- `if seed == ...`
- `if external_seed == ...`
- `match numeric_seed`
- hardcoded card/relic/potion/monster/reward identities for one captured trace
- hardcoded RNG counters for one captured trace
- restoring observed state only for named seeds or trace labels

Fixed seeds are allowed only inside tests, documentation, or corpus metadata as
ordinary deterministic fixtures. Test fixtures must not change production
behavior.

When a trace exposes a mismatch, fix the generic mechanic, RNG stream, state
carry, mapping, or command translation that caused it. If the generic behavior
is not yet understood, leave the trace failing with a documented blocker rather
than adding a seed-specific workaround.

Verifier diagnostics may report observed identities and inferred counters, but
they must not use those observations to alter authoritative replay behavior for
that seed.

## Trace-First Fidelity Testing

For simulator fidelity bugs found through live play or CommunicationMod replay,
prefer persistent trace replay coverage over new narrow unit tests. The trace is
the primary evidence that the simulator still matches the real game.

Use unit tests sparingly for simulator mechanics. They are appropriate for
infrastructure, parsers/mappers, serialization, deterministic invariants, or a
small source-backed rule that a trace cannot isolate cleanly. Avoid broad
gameplay unit tests that simply encode an agent's current interpretation of the
game; they can make the wrong model look authoritative.

## Verifier Workflow

Use the Rust verifier for simulator and verifier iteration by default. Do not
use the UI as the test loop for trace mismatches, and do not restart the UI just
to check a simulator fix.

Preferred checks:

- `cargo check -p sts_core --lib`
- `cargo check -p sts_verify --lib`
- `uv run -- cargo run -p sts_verify --bin sts_verify -- parity <trace.jsonl>`
- focused Rust tests for source-backed mechanics when a full trace cannot yet
  isolate the rule

Only use Python strict replay when it exposes a full-trace replay surface that
the Rust CLI does not yet expose. Only rebuild the Python extension and restart
the UI when the user needs to keep playing with newly compiled native code.
