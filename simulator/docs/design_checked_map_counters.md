# Checked Map Counters

## Problem

Map transitions silently saturated Wing Boots charges and normal/elite combat
counts. Event-room resolution used unchecked arithmetic for Tiny Chest and the
three accumulated room-type chances. Invalid imported state could therefore
become a plausible transition, panic in debug builds, or consume event RNG
before reporting a later counter failure.

## Decision

Authoritative map-owned counters use checked arithmetic and return
`InvalidState` when their next value is unrepresentable. Normal and elite
combat counts are preflighted before encounter generation or combat entry.
Wing Boots decrement remains behind the existing legal traversal proof but
also checks that proof at mutation time. Tiny Chest rejects counters outside
its stable pre-threshold domain.

Event-room resolution is clone-and-commit. Its RNG draw, Tiny Chest update,
chance updates, and selected room transition either all succeed or leave the
exact input state and RNG counter unchanged. The three chance results are
proved before any is assigned. Pure probability-bucket sums use `u64`, which
preserves ordinary `u32` behavior while avoiding arithmetic failure during
classification.

Capacity clamps, index calculations, and explicitly bounded local combat
counters remain saturating where saturation is their intended domain behavior;
this change does not mechanically replace unrelated saturation.

## Verification

Focused regressions cover normal and elite combat-count overflow, invalid Tiny
Chest state, atomic chance-update failure, public event-room rollback including
its RNG counter, and widened probability-bucket classification. Workspace and
permanent replay gates protect supported map, encounter, and event sequencing.
