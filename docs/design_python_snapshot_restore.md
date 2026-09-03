# Python snapshot restoration

## Supported contract

`sts_sim.RunEnv.from_snapshot()` is the supported restoration API. Its input
includes an explicit schema version, accepts only the current snapshot schema
(version 8), and must pass `RunState::validate()` before the environment is
published. Historical schema versions are rejected; there is no named validated
legacy schema.

## Debugging-only raw import

Raw unversioned `full_state()` JSON is a detached diagnostic dictionary. It is
not a restoration API. Tests and callers that need a mutated combat must edit
current schema-8 snapshot JSON and restore it through
`RunEnv.from_snapshot()`, which rejects unknown fields, aliases, and
noncanonical skipped defaults.

The former ambiguous `from_state_json()` / `from_state_json_for_debugging()`
names are not retained: unversioned state must never look like a supported
persistence format.
