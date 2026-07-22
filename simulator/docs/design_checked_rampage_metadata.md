# Checked Rampage Metadata

## Problem

Rampage stores combat-local damage growth on each card instance. Imported state
could attach that bonus to another card, make it negative, or retain it in the
run deck. Rampage queue construction also treated a missing hand card as zero
bonus and added base damage with unchecked signed arithmetic. Growth used a
generic checked addition but did not verify that its source was Rampage.

These paths could turn malformed or divergent state into plausible damage, or
panic or wrap when a representable bonus was near the signed integer limit.

## Decision

Combat validation requires Rampage bonuses to be nonnegative and permits a
nonzero bonus only on Rampage or Rampage+. Run validation rejects every nonzero
Rampage bonus because the field is combat-local. The same rule applies to run
card choices at the import boundary.

Rampage queue construction now requires the hand card and static damage
metadata, checks base damage plus the instance bonus, and returns a
field-specific `InvalidState` on overflow. Growth requires a Rampage source and
uses a field-specific checked addition. The cloned combat-action boundary returns
no partial state when queue construction fails. The growth mutation is
independently atomic if an invalid internal action is ever constructed.

## Verification

Regressions cover combat and run metadata validation and reachable damage
overflow at the public action boundary. Growth overflow and a non-Rampage source
are covered directly at the internal mutation boundary; growth overflow is not
reachable after checked damage because Rampage's base damage is at least as large
as its growth. Existing Rampage and Rampage+ behavior remains required.
Formatting, strict workspace Clippy, workspace tests, snapshot round trip, and
repeated permanent-corpus replay remain commit gates.
