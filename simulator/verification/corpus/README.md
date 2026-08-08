# CommunicationMod verification corpus policy

The active CommunicationMod replay corpus is schema-v1-only as of 2026-08-07.
Schema-v0 compatibility ended at aggregate archive manifest SHA-256
`73b3243eb4bbf05396c19ec87a9e0d7da19848c843dadae13e8095dc536f5202`.

## Required input contract

Every supported trace must have one leading metadata record with:

- integer `schema: 1`;
- `source: "communication_mod"`;
- integer `boundary_schema: 1`;
- typed `run_config.profile.note_card` and non-negative integer
  `run_config.profile.note_upgrades`.

Every state must independently declare integer `boundary_schema: 1` and a
supported authoritative `boundary_kind`. `STATE` closes only on `poll`;
gameplay commands close only on `interaction_ready`, `quiescent`, or
`terminal`. Action steps start at one, are exact contiguous JSON integers, and
each action is completed immediately by a same-step state or error. Typed
`external_rng` records may occur only on their still-pending same-step action.
Boolean, fractional, stringified, missing, mixed, and unknown schema values are
invalid.

Schema-v0 or metadata-incomplete input returns an explicit unsupported-schema
error before simulator replay. There is no fallback, conversion, folding,
observation hydration, candidate-state selection, ignored tail, or deferred
frame interpretation.

## Active directories

- `permanent_traces/`: genuine clean-through-EOF parity passes only.
- `fidelity_regressions/`: strict schema-v1 targeted regressions only.
- `open_failures/`: structurally valid schema-v1 traces with honest simulator
  divergence or unsupported mechanics.
- `live_trace_fixtures/`: non-CommunicationMod session-format fixtures; these
  are not parity replay inputs.
- `manual/`: simulator fixtures, not CommunicationMod traces.

A trace may enter `permanent_traces/` only after structural validation,
normalization idempotence, full action-integrity accounting, deterministic
replay, and a genuine `complete_pass`. Retiring old traces or moving a failure
never establishes parity. Failures remain failures until the simulator-owned
transition matches without accommodation.

## Forensic archives

The immutable schema-v0 archive is external to the repository and all discovery
paths:

`/home/davton/archives/slay-the-spire/schema-v0-2026-08-07/`

Together with the read-only minimized supplement at
`/home/davton/archives/slay-the-spire/schema-v0-minimized-supplement-2026-08-08/`,
it contains 1,258 byte-verified CommunicationMod traces. Aggregate manifest
SHA-256 is
`73b3243eb4bbf05396c19ec87a9e0d7da19848c843dadae13e8095dc536f5202`.
The durable manifest and verification evidence are in
`simulator/verification/schema_v0_archive_2026-08-07/`; verification is
reproducible with `tools/communication/archive_schema_v0_traces.py verify`.
Archived files are immutable forensic evidence, not supported replay inputs.

A separate immutable archive preserves 113 early state-schema-v1 traces that
predate the final explicit metadata/profile policy:

`/home/davton/archives/slay-the-spire/schema-v1-pre-policy-2026-08-07/`

Its manifest SHA-256 is
`d7ff7f99d0dc9d375497833ec14de5369f0632bf1f4fadeb539d09a94aeabe48`.
