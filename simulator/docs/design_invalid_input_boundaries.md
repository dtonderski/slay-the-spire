# Invalid-input replay boundaries

## Problem

Some trace defects are discovered only after seed-start replay has begun. For
example, Neow reward ordering depends on command timing that can be absent or
malformed in an otherwise parseable trace. Replay records that stop as a
`SeedStartBoundary` with category `invalid_input`.

Outcome assessment previously treated every in-report boundary as either an
expected boundary or a generic verification failure. That allowed malformed
input to be declared an `expected_boundary` success when a manifest named it,
and otherwise hid the explicit invalid-input classification behind
`unexpected_boundary`.

## Contract

A consistent replay boundary with category `invalid_input` always produces the
typed `VerificationOutcome::InvalidInput`, independently of the corpus
expectation. Invalid input is not simulator coverage and cannot be accepted as
an expected boundary.

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
`invalid_input` boundary without risking an expected-boundary pass. Unknown but
well-formed display-only card identities remain a separate visibility/content
question and must not be conflated with malformed trace input.
