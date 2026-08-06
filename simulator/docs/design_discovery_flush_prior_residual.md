> **Historical note — superseded.** The pending-stage and stacked-Discovery
> flush below depended on an abandoned cross-command residual model. It is
> retained as historical evidence; see
> [design_discovery_action_update_lifecycle.md](design_discovery_action_update_lifecycle.md)
> for the current source-backed lifecycle.

# Flush prior Discovery residual on stacked hand-played Discovery

## Problem

`pending_hand_discovery_card_reward_stage` is a single slot. Choosing a second
hand-played Discovery while prior SuperFastMode residual stages are unfinished
would overwrite the stage and drop remaining burns.

## Model

Before arming stage 6 for a new hand-played Discovery pick, call
`flush_pending_hand_discovery_card_reward_rng`, which repeatedly forces
`end_turns_remaining = 1` and runs `settle_pending_hand_discovery_card_reward_rng`
until stage returns to 0.

## Status

Defensive correctness for dual Discovery combats (FIDL00411). Does not by itself
clear FIDL00411 Blind/Mind Blast Magnetism residual. Permanent corpus remains green.

## Rejected: burn 6 gens on Discovery→END stage-6 path

Burning the `before_non_end` six generations when stage 6 meets END (no non-END
in between) breaks permanent FIDL00226 Magnetism ordering. That path must keep
skipping the six gens and relying on staged END residual only. Dual-oracle vs
traces that appear short after Discovery→END (FIDL00411/405).
