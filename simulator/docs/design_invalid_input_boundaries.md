# Invalid-input replay boundaries

## Problem

Some trace defects are discovered only after seed-start replay has begun. For
example, Neow reward ordering depends on command timing that can be absent or
malformed in an otherwise parseable trace. Replay records that stop as a
`SeedStartBoundary` with category `invalid_input`.

Outcome assessment previously allowed a manifest declaration to turn a known
replay stop into success. That could hide malformed input behind a passing
corpus entry instead of preserving its explicit invalid-input classification.

## Contract

A consistent replay boundary with category `invalid_input` always produces the
typed `VerificationOutcome::InvalidInput`, independently of the corpus
expectation. Invalid input is not simulator coverage and cannot pass.

The classification is fail-closed with respect to verifier consistency. The
seed-start report must also set `failed = true`. If the flag and boundary
category disagree, assessment returns `VerificationOutcome::Failed` with
`InconsistentBoundaryStatus`; it must not reinterpret a contradictory report
as an input error.

Invalid input is decisive even if replay accumulated other partial evidence.
Such evidence remains available in the report, but no parity verdict is valid
for a malformed trace. CLI and corpus status continue to map invalid input to
their distinct nonzero exit status.

## Follow-up

Observed-card parsing can now route malformed values into an in-report
`invalid_input` boundary without risking a pass. Unknown but well-formed
display-only card identities remain a separate visibility/content question and
must not be conflated with malformed trace input.
