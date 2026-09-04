# Fair Observation Hidden-State Audit

Source of truth:

- `simulator/crates/sts_core/src/combat/fair_observation.rs`
- `simulator/crates/sts_core/src/run/fair_observation.rs`
- `simulator/crates/sts_core/src/run/player_choice.rs`

The current projection has no known hidden-state leak. Existing tests require
byte-identical observations and choices after hidden pile permutations, RNG
changes, internal-ID renumbering, hidden Runic Dome intent changes, and private
queue/relic/monster mutations. They also require public HP, hand order, pile
membership, visible intent, Frozen Eye order, and gold changes to affect output.

## Classification

- **Public:** currently visible in the game UI. May be emitted directly.
- **Public history:** derivable from prior public events. May be emitted only
  from an explicit public-history record, never reconstructed from hidden state.
- **Latent:** hidden but covered by a declared source-backed prior. May be
  sampled independently of the true simulator state.
- **Forbidden:** hidden without such a prior, or internal identity/queue
  scaffolding. Must be refused rather than copied or inferred.

## Hidden fields

The fair boundary excludes RNG streams and counters, unknown pile order,
internal card/content/monster IDs, private monster move history and counters,
unrevealed intent, action queues, limbo, pending decisions/effects, future room
and reward contents, process-global RNG, and snapshots.

Unknown draw/discard/exhaust order may be sampled only by an explicitly named
belief model. Publicly revealed order is not sampled. Runic Dome intent needs a
source-backed move model conditioned on public history; absent that model, fair
belief construction must refuse the state.

## Known underexposure

The projection currently omits some information a player could track, including
public placement history, general combat turn number, public monster move
history, next-turn energy/retention, and some later-act powers. This limits agent
strength but does not leak hidden state.

`stasis_card` is projected when present, and Nilry's Codex currently shares the
Toolbox selection kind. These are documented representation choices, not known
leaks.

## Rule

A fair consumer receives only fair observations, public choices, and explicit
public history. Full `RunState`, snapshots, RNG, or generated hidden hypotheses
remain verifier/teacher/planner-internal and must never become policy input.
