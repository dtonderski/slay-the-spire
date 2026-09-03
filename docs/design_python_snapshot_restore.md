# Python snapshot restoration

## Supported contract

`sts_sim.RunEnv.from_snapshot()` is the supported restoration API. Its input
includes an explicit schema version, accepts only the current snapshot schema
(version 8), and must pass `RunState::validate()` before the environment is
published. Historical schema versions are rejected; there is no named validated
legacy schema.

## Debugging-only raw import

`from_state_json_for_debugging()` remains available for diagnostics that need
to inspect current in-memory JSON. It is deliberately named as a debugging
surface, carries no schema-compatibility guarantee, and still validates every
imported state. Callers that persist or resume environments must use versioned
snapshot JSON instead.

The former ambiguous `from_state_json()` name is not retained: unversioned
state must never look like a supported persistence format.
