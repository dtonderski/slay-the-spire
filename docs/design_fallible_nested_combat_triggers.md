# Fallible Nested Combat Triggers

## Problem

Player debuffs can be applied several levels below an authoritative action:
Fire Breathing can kill a Fungi Beast while drawing a curse, Spore Cloud then
applies Vulnerable, and Gremlin Horn can draw another card during the same
death trigger. End-turn curses and Dark Embrace add parallel draw and HP-loss
paths. These helpers currently return `()`, so a future checked debuff cannot
propagate `InvalidState` without a panic, ignored result, or duplicated
preflight arithmetic.

Direct block triggers form another branch of the same chain: Rage, Feel No
Pain, Abacus, Metallicize, Plated Armor, Self-Forming Clay, and Orichalcum can
invoke Juggernaut; Juggernaut can kill a monster; and that death can invoke
Spore Cloud and Gremlin Horn. Shuffle helpers must therefore propagate Abacus
trigger failures as well as draw-trigger failures.

Gremlin Horn also adds energy with wrapping arithmetic before its nested draw.
An unrepresentable imported counter can therefore become plausible combat
state inside the same trigger chain.

## Decision

Card draws, Fire Breathing, monster-death hooks, monster-death relics,
on-exhaust effects, HP-loss hooks, end-turn hand resolution, and the start/end
turn relic surfaces that invoke them return `SimResult`. Direct block,
Juggernaut, and shuffle-trigger surfaces do the same. Every existing caller
propagates the typed failure to its already cloned combat/run transition.

The externally callable draw helpers stage both combat state and any supplied
RNG, committing only after the complete recursive draw succeeds. Immediate
monster-death hooks likewise stage the combat state before applying non-relic
death effects and Gremlin Horn. Queued death hooks keep their existing queue
semantics and rely on the enclosing action-queue candidate for rollback.

Gremlin Horn proves its energy addition before assignment, then performs its
draw. No valid-path trigger ordering, RNG consumption, draw recursion, or
deferred death-relic behavior changes.

This is propagation plumbing required by the checked player-debuff boundary.
It does not claim to audit unrelated HP-loss, healing, or relic arithmetic;
those remain separate fail-closed slices.

## Verification

Regressions require Gremlin Horn energy overflow, including a Juggernaut kill
reached through Rage, to return `InvalidState` at the combat action boundary
without mutating the input state. Representable death/draw/exhaust and end-turn
behavior remains covered by existing tests and permanent traces. The full
workspace, strict corpus, snapshot round trip, and repeated permanent replay
gates remain required before commit.
