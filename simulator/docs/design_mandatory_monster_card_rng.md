# Mandatory monster card RNG

## Problem

Monster intent execution accepted an optional `cardRandomRng`. Without it,
random draw-pile insertion appended cards deterministically and Bronze Orb
stasis silently selected nothing. Both branches returned successful combat
transitions with different mechanics.

## Decision

- Require `&mut StsRng` for random draw-pile insertion.
- Require the same stream for every authoritative monster-intent execution,
  even when a particular intent does not consume it.
- Remove the public no-RNG monster-intent wrapper.
- Require RNG for Bronze Orb stasis selection rather than treating absence as
  an empty eligible pile.

## Verification

A deterministic insertion regression proves that nonempty draw piles consume
the stream and preserve unique card IDs. Existing monster, turn, strict corpus,
repeated replay, and snapshot gates remain required before commit.
