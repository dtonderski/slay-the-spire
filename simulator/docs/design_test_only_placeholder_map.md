# Test-only placeholder map

## Problem

`sts_core::map::generation` exposed a seven-node simulator-only map generator
and three compatibility fixture wrappers from the production crate. The module
explicitly did not model target-game map generation, and repository search found
no production consumer. Two integration tests used it to exercise deterministic
traversal and the Ascension 1 elite-room toggle.

Keeping that generator public made a deliberately invented topology look like a
supported run-construction path. It also duplicated the target-backed map
generation already used by seeded run construction.

## Decision

- Remove the placeholder generator module and all four production exports.
- Preserve the deterministic helper under `sts_core/tests/support`.
- Keep both existing behavioral tests on that test-local helper.
- Do not change explicit milestone fixtures or target-backed map generation in
  this slice.

## Verification

The focused milestone 8 and milestone 11 tests must pass. Workspace formatting,
Clippy, all-target/all-feature tests, strict corpus repetitions, and snapshot
round trips remain required before commit.
