# Fail-closed Mayhem turn startup

## Problem

Start-of-turn Mayhem treated any top-card execution error as a reason to stop
processing the power and continue into a normal player turn. Unknown content,
unsupported mechanics, invalid targeting, or missing authoritative state could
therefore become a plausible success. Unknown top-card definitions were also
silently treated like an empty draw pile.

`start_player_turn` mutated its argument in place and returned no result, so its
callers had no channel for preserving those failures.

## Decision

- Make `start_player_turn` return `SimResult<()>`.
- Apply turn startup to a clone and publish it only after the whole transition
  succeeds, leaving the caller's state unchanged on error.
- Distinguish an empty draw pile from unknown top-card content.
- Reject known cards missing from the top-draw dispatcher with
  `UnsupportedMechanic` instead of completing a no-effect play.
- Preserve the explicit autoplay contract for unplayable curses/statuses:
  their effect is skipped, while Havoc/Mayhem applies its required pile movement.
- Enumerate playable cards with intentionally empty effects, such as Slimed,
  explicitly so they cannot reopen a generic no-effect fallback.
- Propagate Mayhem top-card execution errors through player-turn startup,
  end-turn combat transitions, and the run-owned combat boundary.

## Verification

Regression tests place unknown and known-but-unimplemented content under an
active Mayhem power. They prove turn startup returns `UnknownContent` or
`UnsupportedMechanic` without partially mutating combat state. Existing
start-turn callers must explicitly handle success. The complete workspace,
strict corpus, deterministic replay, and snapshot gates remain required before
commit.

The permanent CODEX10 trace previously passed only because Havoc's top-drawn
Double Tap+ was a no-effect play. With the effect modeled, its following
Uppercut+ post-state exposes the original hit and decremented Double Tap counter
but never records the queued copy before another command. The verifier defers
the assertion and requires the next pre-state to show the settled copy. This
trace instead issues `END` against the same incomplete state, so the verifier
reports `unreconciled_copied_attack_frame` at that command and the corpus
declares the boundary instead of claiming complete parity.
