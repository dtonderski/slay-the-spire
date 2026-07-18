# Bronze Automaton repeating move cycle

## Problem

The simulator only scheduled Hyper Beam when `moves_executed == 5`. The target
boss resets its private `numTurns` counter after every Hyper Beam, so long
combats repeat the beam cycle. Session-1204 reached the second beam and the
simulator incorrectly used Flail, underpredicting damage by 19 HP after block.

## Source-backed rule

Target `BronzeAutomaton.getMove()` produces Beam at move indices 5, 11, 17,
and so on. Equivalently, after the opening summon, every move index congruent
to 5 modulo 6 is Hyper Beam. The post-beam move remains Stun below A19 and
Boost at A19+, followed by the existing Flail/Boost alternation.

## Verification

Extend the source-cycle unit test through the second Beam and replay
CommunicationMod session-1204 through its floor-33 `END` transition.
