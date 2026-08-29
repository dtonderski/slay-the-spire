# Checked Pending Relic Energy

## Problem

Happy Flower can grant energy on the first player turn while Toolbox still has
an opening card choice active. The simulator defers that energy until the
Toolbox choice closes. The deferred counter is validated as nonnegative, but
settlement used unchecked signed addition and cleared the counter as part of
the same expression.

An imported state can therefore overflow player energy when the choice is
resolved. Debug and release builds must not disagree, and an invalid settlement
must not consume the Toolbox choice or insert its selected card.

## Decision

Pending start-of-turn relic energy is added with `checked_add`. The pending
counter is cleared only after the sum succeeds. Overflow returns a specific
`InvalidState` error.

The Toolbox choice transition already stages a cloned `RunState`, so propagating
the settlement error preserves the complete authoritative input state. Valid
choice ordering and Happy Flower timing remain unchanged.

## Verification

A regression imports a representable pending counter beside maximum player
energy and verifies that resolving Toolbox fails without changing the run.
Formatting, strict workspace Clippy, workspace tests, snapshot round trip, and
repeated permanent-corpus replay remain commit gates.
