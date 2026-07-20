# Dream Catcher reward continuation

## Problem

The verifier handled `SKIP` on Dream Catcher's rest-site card reward by clearing
reward fields directly. It then inspected the observed post-screen to decide
whether to rewrite the authoritative simulator phase back to `Rest`. This made
the observation control simulation and compensated for a missing core
continuation rule.

## Core rule

Choosing `Heal` completes the rest-site action before Dream Catcher opens its
card reward. Core records that completion in `rest_room_complete`. When the
card reward is taken or skipped and no reward item remains, reward settlement
returns to `RunPhase::Rest` and clears the reward screen. The normal typed
`RestAction::Proceed` then leaves the completed rest room.

Core `CloseCardReward` distinguishes this completed rest continuation from an
ordinary combat reward: combat `SKIP` closes the overlay while retaining the
underlying card reward item, whereas Dream Catcher consumes the standalone
offer and returns to `Rest`.

The verifier binds `SKIP` to core `RunAction::CloseCardReward`, chooses its
projection and next replay phase from the resulting core state, and compares
the corresponding observed projection. The observed post-screen is never
assigned into simulator state or used to choose the phase.

Neow's completed stage-two continuation is a distinct visibility case: core is
already in the typed `Event` continuation, while the command-facing game frame
remains an empty combat-reward screen until `PROCEED` leaves Neow's room. The
verifier projects that frame only when core identifies Neow stage two; it does
not inspect the observed screen to select the projection.
