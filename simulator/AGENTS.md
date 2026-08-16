# Simulator Agent Rules

These apply to all work under `simulator/`, in addition to the root
`AGENTS.md`. Testing policy, determinism, and the Never list live there; this
file carries the enforcement detail for seed-specific workarounds.

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

## Verifier Workflow

Beyond the parity and status commands in the root `AGENTS.md`:

- `cargo check -p sts_core --lib` and `cargo check -p sts_verify --lib` for fast
  feedback.
- `sts_verify status` defaults to `verification/corpus/permanent_traces/` and
  caps at 24 workers, bounded by available CPUs.
- Rebuild the Python extension and restart the UI only when the user needs to
  keep playing with newly compiled native code.
