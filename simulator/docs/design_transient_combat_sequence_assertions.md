# Transient combat sequence assertions

> **Historical and superseded.** Boundary schema v1 removed transient combat
> sequences, pending dispositions, and deferred reconciliation. Intermediate
> same-step states are invalid rather than foldable verification evidence.

## Problem

CommunicationMod can expose a command-ready `GRID` or `HAND_SELECT` while its
action manager still reports `EXECUTING_ACTIONS`. The verifier previously
deleted player HP, block, energy, and the complete monster projection from both
sides, compared the remainder, and counted the originating combat action as
verified. That overstated coverage and made those gameplay fields disappear
without later reconciliation.

These frames cannot always be folded out by trace pairing. A selection screen
may accept `CHOOSE` and `CONFIRM` before the action queue settles, so several
real semantic commands can occur inside one transient interval.

## Contract

Core state advances for every typed combat command. An executing selection
frame compares only the immediately visible selection contract: screen type,
piles, cards, potions, floor, gold, and other fields not explicitly deferred.
Player HP, block, energy, and monsters remain pending; they are not erased from
the verification claim.

Each command whose post-state is executing joins one pending combat sequence.
Observation polls and further selection commands may extend that sequence. The
first non-executing combat frame receives a full core-owned comparison. Only if
that stable comparison and every earlier transient comparison match are the
pending actions marked verified and tagged as reconciled deferred assertions.
Ending the trace before that point leaves one unresolved assertion per pending
action, and each receives an explicit `pending_transient` action disposition
instead of being counted as verified or left unclassified. A stable mismatch
remains a normal diff for the settling command and prevents the earlier actions
from being reported as reconciled.

The observation never mutates core state or chooses the stable projection.

## Evidence

The regression derives from the committed session-38 Hex/Armaments trace. It
contains `PLAY`, `STATE`, `CHOOSE`, `STATE`, and `CONFIRM` across command-ready
executing `HAND_SELECT` frames before returning to stable combat. A truncated
prefix must remain unresolved, the full sequence must reconcile, and a forged
transient visible field must remain a diff even if the later stable state
matches.
