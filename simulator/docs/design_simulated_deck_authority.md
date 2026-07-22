# Simulated deck authority

## Problem

The seed-start verifier keeps a `deck_ids` list alongside `RunState::deck`.
During Neow this list represents real target update-order distinctions: a curse,
transform result, or selected reward card can become visible on a later frame
than the authoritative transition. After Neow room entry settles, however, the
list is never read. Replay code repeatedly copies the core deck into it, leaving
a second stable state variable that appears authoritative without affecting any
comparison or transition.

## Contract

After Neow room entry, `RunState::deck` is the sole stable simulated deck
authority. Stable screen projectors read it directly. Replay phases must not
maintain a verifier-owned stable deck mirror.

Early Neow staging remains local and explicit until that update-order behavior
is represented by typed replay phases. Every staged deck is derived from the
typed start command, pre-command simulator state, deterministic mechanics, and
recorded command timing. Observed post-state never selects or mutates a staged
deck.

## Transient frames

A pre-command deck or action-frame deck may be retained only inside a named
deferred assertion. It must name the settled core deck and reconcile on a later
stable frame. An unresolved or divergent assertion prevents a passing outcome.
This preserves legitimate CommunicationMod lag without turning transient state
into a second simulator authority.

## Failure behavior

Missing core state, impossible update ordering, or a post-Neow transition that
cannot project from `RunState` is a typed boundary or mismatch. The verifier
must not reconstruct a stable deck from observation or silently fall back to a
carried list.
