# Communication Boundary Metadata MVP

## Problem

CommunicationMod currently publishes an input screen as soon as it opens, even when a combat action remains active. The bridge treats any new state as command completion, and the random collector then guesses which later `STATE` poll is the settled response by comparing visible JSON. This loses action-update timing and forces strict replay to consider multiple timing interpretations.

## Minimal design

Preserve the existing lockstep text-command protocol. Add diagnostic metadata to every CommunicationMod state:

- `boundary_schema = 1`;
- `boundary_kind = interaction_ready | quiescent | terminal | poll`;
- cumulative game and dungeon update counters;
- current action class, process-local action-instance sequence, and update count;
- action, card, and pre-turn queue sizes.

`interaction_ready` means a screen requires input while the action manager is not quiescent. `quiescent` means the action manager has no current action or queued work. `poll` is an explicit `STATE` observation and is not gameplay settlement. `terminal` covers out-of-run/death boundaries.

CommunicationMod can emit a natural state while a command is still crossing the process pipe, so arrival order alone is not causal order. The trace client therefore keeps one command in flight: `STATE` completes only on `poll`, while gameplay completes only on `interaction_ready`, `quiescent`, or `terminal`. Overtaking states are published as diagnostics without consuming another command. Explicit poll markers are consumed by the first serialized response and cleared by any subsequent gameplay command, preventing a poll from leaking across commands. Command envelopes are not required for this MVP because the declared boundary kind closes the single in-flight command.

## Collector behavior

For schema-1 states, the collector accepts only `interaction_ready`, `quiescent`, or `terminal` as the response to a gameplay command and only `poll` as the response to `STATE`. It must not use semantic JSON changes to choose a later state. Schema-1 normalization uses only those declared command/boundary rules to omit overtaking transport states. Missing or unknown final boundary metadata fails closed. Legacy semantic settlement folding remains isolated to old schema-0 traces.

Update counters are recorded timing evidence. They do not authorize verifier state hydration or hidden RNG synchronization. If simulator behavior depends on external update count, replay must model that count as explicit timing input or classify the witness under campaign policy.

## Focused validation

A live target-game diagnostic exercised startup, an explicit `STATE`, and Forethought hand selection. Startup and settled gameplay emitted `quiescent`, the explicit observation emitted `poll`, and the open hand selection emitted `interaction_ready` with `current_action=ForethoughtAction`, a stable action-instance ID, update count 1, and one queued action. Closing the selection returned to `quiescent` with empty queues.

End-to-end validation then collected `BNDARYFULL2` through a terminal boundary (1,476 actions/states), followed by continuous campaign traces `FIDL01041` and `FIDL01042`. Every retained state used `boundary_schema=1`; gameplay had zero poll pairings; queue/action metadata validated; and schema-1 normalization was idempotent without semantic settlement selection.
