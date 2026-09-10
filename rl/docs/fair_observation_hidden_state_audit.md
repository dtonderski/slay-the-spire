# Fair Observation Hidden-State Audit

Source of truth:

- `simulator/crates/sts_env/src/combat_observation.rs`
- `simulator/crates/sts_env/src/run_observation.rs`
- `simulator/crates/sts_env/src/action.rs`
- `simulator/crates/sts_core/src/combat/pile_knowledge.rs`

The current projection has no known hidden-state leak. Existing tests require
byte-identical observations and choices after hidden pile permutations, RNG
changes, internal-ID renumbering, hidden Runic Dome intent changes, and private
queue/relic/monster mutations. They also require public HP, hand order, pile
membership, visible intent, Frozen Eye order, public draw-position history, and
gold changes to affect output.

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
general combat turn number, public monster move history, next-turn
energy/retention, and some later-act powers. This limits agent strength but does
not leak hidden state.

Draw-pile `known_positions` is an explicit public-history record updated at
accepted transitions. Supported history includes top inserts (Headbutt, Warcry,
Thinking Ahead, add-to-top), bottom inserts (Forethought), draws and PlayTop
removals, and Scry prefix reveals when the overlay opens. Conservative
invalidation applies to shuffles, random inserts (Wild Strike, Hex, Mark of
Pain, Nilry, monster status), unknown-index removals (Secret Technique/Weapon,
Violence, Scry discard, Bronze Orb Stasis from the draw pile), Distilled Chaos
interrupt restores of unplayed hidden tops, and Toolbox parking of the unseen
opening draw. Distilled Chaos currently-played push/pop is a public PlayTop, not
a reveal of unplayed hold-outs. Frozen Eye is a view-time overlay and does not
write the tracker. Discard and exhaust `known_positions` stay empty in this
schema. Unknown-position removals do not branch on private `CardId`. Complex
exact tracking may be coarser than a player who memorized every public reveal;
missed unknown mutations invalidate rather than guess. Historical in-combat
snapshots taken before this field restore with empty knowledge and are not
repaired from hidden pile order.

`stasis_card` is projected when present, and Nilry's Codex currently shares the
Toolbox selection kind. These are documented representation choices, not known
leaks.

## Rule

A fair consumer receives only fair observations, public choices, and explicit
public history. Full `RunState`, snapshots, RNG, or generated hidden hypotheses
remain verifier/teacher/planner-internal and must never become policy input.
