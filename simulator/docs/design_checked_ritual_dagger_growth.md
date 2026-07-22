# Checked Ritual Dagger Growth

## Problem

Ritual Dagger adds its per-instance bonus to base damage, then permanently grows
that bonus after a lethal non-minion hit. Both additions used unchecked signed
arithmetic, and growth silently did nothing if the source card could not be
found in an authoritative combat pile.

An imported state with a large representable bonus could therefore panic or wrap
while constructing damage or after an otherwise legal lethal play. Silently
losing growth for a missing source card would also turn divergent state into a
plausible continuation.

## Decision

Ritual Dagger damage construction and growth are fallible. Base damage plus the
instance bonus uses `checked_add`, and missing static damage metadata is invalid.
The effect queue rejects a non-Ritual-Dagger card instead of substituting
definition defaults. After a lethal hit, the source card must exist in hand,
draw, discard, or exhaust; otherwise the transition returns `UnknownCard`.
Positive growth also uses `checked_add`. Both overflow paths return
field-specific `InvalidState` errors.

The existing cloned combat-action boundary commits neither lethal damage nor
card movement when growth fails. Minion kills still do not grow the card, and
normal and top-draw plays continue to share the same growth action.

## Verification

Regressions cover reachable damage overflow at the public combat-action boundary
and directly cover growth overflow plus a missing source card without mutation.
Existing normal, upgraded, minion, run-deck transfer, and top-draw Ritual Dagger
tests remain required. Formatting, strict workspace Clippy, workspace tests,
snapshot round trip, and repeated permanent-corpus replay remain commit gates.
