# Internal action dispatcher split

## Problem

`apply_internal_action` is the authoritative executor for queued combat actions.
Its large match mixes unrelated action families, which makes review and future
mechanics work harder, but changing its ordering or error behavior would risk
combat parity.

## Contract

Structural extractions from this dispatcher must preserve:

- the exact `InternalAction` variant matched by each arm;
- mutation and early-return ordering within every arm;
- the exact follow-up action sequence;
- existing typed errors and their triggering conditions;
- queue insertion and queue-drain behavior.

The parent dispatcher remains the single routing authority. Family modules may
mutate `CombatState` and return follow-ups, but they do not drain or reorder the
queue.

## First slice: decision-opening actions

Move the implementations for `AwaitHandSelect`, `AwaitDrawSelect`,
`AwaitDiscardSelect`, `AwaitExhaustSelect`, and `OpenDiscoveryCardReward` into a
private sibling module. Keep thin, explicit match arms in
`apply_internal_action`; do not add a fallback dispatcher or a second action
classification.

This slice is mechanical: it introduces no new state representation, legal
action, fallback, or compatibility behavior.

## Verification

Review the moved bodies against the pre-extraction diff, run focused `sts_core`
tests, then run formatting, strict workspace Clippy, snapshot round trips,
workspace tests, and repeated permanent-corpus replay at the clean commit
boundary.

## Second slice: pile and draw actions

Move the implementations that mutate card piles, generate cards, shuffle, or
draw into a private `pile_actions` module. The parent match continues to name
and route every `InternalAction` variant explicitly. Helpers retain the exact
source-card removal, relic trigger, RNG-consumption, and follow-up ordering from
the inline arms.

## Third slice: player-local actions

Move player resource changes, hand-card metadata changes, and player power
mutations into a private `player_actions` module. Keep damage, exhaust hooks,
and top-draw execution outside this family so their queue-sensitive sequencing
remains independently reviewable. The parent match continues to route every
variant explicitly.
