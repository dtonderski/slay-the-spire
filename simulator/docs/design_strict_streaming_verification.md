# Strict schema-v1 streaming verification

## Decision

CommunicationMod parity and replay consume JSONL incrementally. JSONL remains
the forensic source of truth; the verifier retains only the authoritative
simulator state, report/accounting output, one pending action and its typed RNG,
and the immediate completing state or error.

The leading nonblank record must be the unique schema-v1 CommunicationMod
metadata record with typed profile input. Unsupported schema is rejected from
that record before any body record is read. Action steps start at one and are
contiguous. External RNG belongs only to the pending same-step action. `STATE`
completes only on `poll`; gameplay completes only on `interaction_ready`,
`quiescent`, or `terminal`.

A semantic difference freezes simulator execution and the first boundary, but
strict mode continues parsing and structurally accounting through EOF. A bad
tail therefore makes the input invalid even after an earlier simulator
difference. Later valid actions receive deterministic `beyond_boundary`
dispositions. Observations remain comparison input only and never select or
mutate simulator state.

`parity --diagnostic-early-exit` may stop at the first semantic boundary for
interactive diagnosis. Its integrity record has `eof_validated=false`, and the
outcome contract makes that evidence ineligible for a complete pass or corpus
promotion.

The producer applies the same contract. CommunicationMod sends a command state
only when its boundary is completing, rather than emitting an earlier
`unknown` state. The collector still rejects any intermediate same-step `state`
record it receives; it does not scan forward or choose a later observation.

## Comparison and parallelism

Projection values are compared directly as typed `serde_json::Value` trees.
The previous value-to-string-to-value roundtrip was removed while retaining the
same key ordering, array paths, missing-as-null behavior, categories, and reason
rendering.

Status and inventory work use bounded indexed workers. Results are stored and
rendered in sorted input order, independent of completion order. Defaults are
capped at four workers; explicit status overrides are also bounded by CPU count
and a hard cap of eight. File-backed parity, replay, live trace analysis, and
permanent promotion all call the reader API. Live replay's plan-extraction pass
retains metadata and actions rather than full observed states; promotion retains
every EOF, disposition, rejection, duplicate, terminality, and clean-boundary
gate.

## Measured result

Evidence and exact commands live under
`random_traces_loop/schema_v1_streaming_work_2026-08-08/`. On FIDL01288, the
pre-change debug median was 2.32 seconds with 755,136 KiB peak RSS. Final
streaming release median is 0.26 seconds with at most 5,376 KiB in the recorded
runs: 8.9 times faster and 99.3 percent lower peak RSS than the debug baseline.
The frozen 46-trace inventory improved from 187.20 seconds in debug to 20.77
seconds in final serial streaming release mode, or from 382.6 to 3,448.6
actions per second, with all 46 outcomes and first-boundary reports identical.
One-, two-, and four-worker status output was byte-identical; four workers took
6.61 seconds versus 18.90 seconds for one.

The deployed release binary has SHA-256
`2ca3daed3c93b988e10be7330b9693eeef3122c0c48a0728142d9962d787e9e5`.
Fresh terminal trace FIDL01303 bound that hash, the collector/client hashes, and
the CommunicationMod hash in leading metadata. Strict replay validated it
through EOF with 1,426 applicable and disposed actions, no duplicate
or rejected dispositions, and an honest step-5 failure.
