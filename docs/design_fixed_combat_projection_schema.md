# Fixed combat projection schema

## Problem

The combat comparison helpers projected `ascension`, `deck_ids`, and
`relic_ids` from simulator state, then deleted those fields whenever the
observed projection omitted them. This made the compared simulator schema
depend on the observation and could hide authoritative run-state divergence
during combat.

## Decision

Combat observed and simulated projectors use one fixed schema for stable and
deferred comparisons. In addition to combat-local state, both sides project:

- ascension;
- ordered master-deck identities;
- ordered relic identities.

CommunicationMod exposes these values in authoritative `game_state` frames.
Missing fields therefore project as malformed/default observation data and
produce differences under the existing strict trace parser; they do not remove
the corresponding simulator fields.

Runic Dome intent hiding, target `DEBUG` intent frames, terminal dead-monster
fields, and explicitly transient action-queue fields remain separate visibility
contracts. None of those contracts permits deleting master run-state identity.

The fixed schema exposed a second authority bug: the combat-entry projector
received the verifier's command-derived relic carry, while later combat
projectors silently substituted an empty carry. That dropped used-up Neow's
Lament after its core combat counter reached zero. Target bytecode for
`NeowsLament.atBattleStart` calls `setCounter(-2)`/`usedUp()` at exhaustion and
does not remove the relic, matching committed CommunicationMod traces. Combat
projection therefore requires the same relic carry at every entry, action,
poll, deferred reconciliation, and Smoke Bomb frame; no convenience path may
invent an empty carry.

## Verification

Regression coverage asserts the observed projector includes the fixed fields
and that missing observed authority differs from simulator values. The strict
fidelity and permanent corpora protect stable combat entry, combat actions,
deferred frames, rewards, and retained boundaries.
