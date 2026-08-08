# Burning Pact deferred selection + pending_actions

## Evidence

FIDL00379 steps 1526–1528 (Burning Pact, Dark Embrace, Ink Bottle):

- CONFIRM leaves the selected Wound outside every serialized pile (ExhaustAction
  skipped retrieval) and puts Burning Pact in discard.
- Draw gains three cards (`Defend_R`, `Clothesline`, `Shockwave`): Burning Pact's
  magicNumber (2) plus Ink Bottle's on-play draw parked under the open exhaust
  select as `pending_actions`.
- Dark Embrace does **not** draw (no moveToExhaustPile for the selection).

Ordinary retrieval would exhaust the Wound, queue Dark Embrace, and with Ink
Bottle often overshoot into a fourth draw (`Bash+` still on draw in the real
frame).

## Implementation

`seed_start_burning_pact_deferred_selection_state` already rebuilt deferred
selection (park selected, BP draws, settle source without DE). It now also
drains `ExhaustSelectState.pending_actions` via `process_internal_queue_public`,
matching `confirm_exhaust_select`'s trailing pending drain so Ink Bottle / Hex
follow-ups still fire after skipped retrieval.

## Non-goals

- Do not skip Dark Embrace on ordinary successful ExhaustAction retrieval.
- Do not seed-specifically force Ink Bottle counters.
