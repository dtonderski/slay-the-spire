# Mandatory encounter misc RNG

## Problem

Act 2 encounter expansion accepted an optional `miscRng`. Random Gremlin Leader
minions could not be constructed without it, while deterministic groups happened
to work because they did not draw. This left the production encounter API with
two state contracts for one operation.

## Decision

- Require `&mut StsRng` for all Act 2 encounter expansion.
- Pass the run-owned `miscRng` through production entry.
- Let deterministic groups consume zero draws rather than omit the stream.
- Preserve the existing seeded convenience wrapper with its explicit stream.

## Verification

A Gremlin Leader regression pins both generated minion identities and two RNG
draws. A deterministic Taskmaster regression pins zero misc draws. Full corpus,
repeated replay, and snapshot gates remain required before commit.
