# Boundary Schema v1 and the Schema-v0 Cutoff

## Decision

CommunicationMod verification supports only boundary schema 1. Schema-v0
compatibility ended on 2026-08-07. Retirement does not establish simulator
parity; it removes untrustworthy timing interpretation from the evidence path.

A supported trace has one leading metadata record declaring integer
`schema=1`, `source=communication_mod`, integer `boundary_schema=1`, and typed
profile input. Every state independently declares integer `boundary_schema=1`,
a supported authoritative boundary kind, and non-negative JSON integer update
and queue counters. Action steps are exact contiguous JSON integers beginning at
one. Each action is completed immediately by its same-step state or error;
typed external RNG belongs to that still-pending action.

`STATE` completes only on `poll`. Gameplay completes only on
`interaction_ready`, `quiescent`, or `terminal`. A schema-v0, missing, mixed,
fractional, stringified, boolean, or unknown schema fails with an explicit
unsupported/invalid-schema error before replay.

## Direct replay architecture

Replay has one path:

1. initialize the simulator from START, explicit seed/unlocks, and typed profile;
2. decode the command against simulator-owned legal decisions;
3. attach typed same-action external RNG;
4. execute exactly one authoritative core transition;
5. compare the immediate authoritative boundary projection;
6. stop at the first difference or unsupported command.

The schema selector, legacy transition constructor, delayed action/state
association, poll/confirmation folding, ignored-tail accounting, state-derived
profile fallback, observed-hand rebinding, pile/order hydration, alternate
snapshots, deferred assertions, source-frame settlement, lag-frame probing,
Havoc/Chrysalis candidates, Confusion cost substitution, alternate Neow decks,
and skipped-retrieval variants were deleted. Observations are expected output
only and cannot choose, restore, reorder, or mutate simulator state.

Rejected commands remain exact same-step error dispositions. Every applicable
action receives one disposition; rejected actions remain separately counted.
There is no deferred or ignored disposition class.

## Evidence retention

Full trace payloads and archive inventories are external data. Git retains this
durable schema decision and compact regression fixtures, but not generated
counts, hashes, outcome ledgers, or corpus snapshots. Current evidence must be
computed directly from the external payloads with the verifier revision being
evaluated.

Schema-v0 and pre-policy schema-v1 captures remain unsupported inputs. Moving
or archiving evidence never establishes parity, and observations remain
expected output rather than simulator state.
