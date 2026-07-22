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

## Fourth slice: defense and debuff actions

Move healing, block, temporary defense, vulnerable/weak, and monster-strength
reduction into a private `defense_actions` module. Preserve dead-target no-op
behavior, checked arithmetic, Artifact handling, Sadistic Nature follow-ups,
and the order in which temporary strength reduction is recorded.

## Fifth slice: multi-target damage actions

Move the three all-enemy damage variants into a private `damage_actions`
module before moving the larger single-target variants. Preserve per-hit death
effects, filtering and deferred aggregation of Malleable block, monster order,
and post-damage healing order.

## Sixth slice: ordinary and random-target damage

Move ordinary single-target damage and random-enemy damage before the
kill-reward variants. Preserve target-existence no-ops, RNG consumption,
Lagavulin/Guardian bookkeeping, Malleable and Hand Drill ordering, slime split,
the ordinary arm's queued death hooks, the random arm's immediate death hooks,
and final spike reflection.

## Seventh slice: gold and healing damage

Move Hand of Greed and damage-plus-heal into `damage_actions`. Preserve the
non-minion gold condition and checked accumulation. For the healing variant,
preserve healing immediately after Malleable follow-up creation and before Hand
Drill, slime split, death hooks, and spike reflection.

## Eighth slice: Feed and Ritual Dagger

Move the final kill-growth variants into `damage_actions`. Preserve Feed's
minion and Darkling exclusions, Magic Flower-adjusted healing, checked max-HP
arithmetic, and Red Skull synchronization before death hooks. Preserve Ritual
Dagger's non-minion source-card growth before death hooks. Both retain final
spike reflection ordering.

## Ninth slice: card-play lifecycle actions

Move card-play and copied-card triggers, duplication/relic consumption, energy
spending, and hand-card cost mutation into a private `card_actions` module.
Preserve normal-play trigger order: Enrage, Rage, relics, Mummified Hand,
powers, then hand-card triggers. Preserve copied-play trigger order: Enrage,
Rage, relics, powers, then copied-card triggers; copied play intentionally does
not apply Mummified Hand. Preserve checked card-energy subtraction and the
existing direct `SpendEnergy` semantics.

Keep copied-effect marker no-ops in the central router. This module does not
classify actions or process the queue.
