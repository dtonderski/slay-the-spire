# Python snapshot restoration

## Supported contract

`OmniCombatEnv.from_snapshot_json()` and `OmniRunEnv.from_snapshot_json()` are
the supported restoration APIs. Their input includes an explicit schema
version, accepts only the current schema or a named validated legacy schema,
and must pass `CombatState::validate()` or `RunState::validate()` before the
environment is published.

## Debugging-only raw import

`from_state_json_for_debugging()` remains available for diagnostics that need
to inspect current in-memory JSON. It is deliberately named as a debugging
surface, carries no schema-compatibility guarantee, and still validates every
imported state. Callers that persist or resume environments must use versioned
snapshot JSON instead.

The former ambiguous `from_state_json()` name is not retained: unversioned
state must never look like a supported persistence format.
