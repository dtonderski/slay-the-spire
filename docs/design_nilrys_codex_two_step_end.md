# Nilry's Codex two-step end-turn (FIDL00451)

## Observed CM protocol (FIDL00451)

Each player turn end with Nilry's Codex publishes:

1. `END` → `CARD_REWARD` with hand still held (pre-discard).
2. `CHOOSE`/`SKIP` → still pre-monster; **draw pile length unchanged** (first
   offer is a pure UI pause — no insert). Multiset 507→508 shows **only one**
   new card after the second pick.
3. `END` → hand discarded, **second** `CARD_REWARD` (different three cards);
   monsters have not acted; hand empty.
4. `CHOOSE`/`SKIP` → monsters act, next hand drawn.

Permanent Nilry tips (`d15d31…`, `599f7cd…`) use a **single** offer only.

## Relic-before-power queue

`GameActionManager.callEndOfTurnActions` runs
`applyEndOfTurnRelics` (`NilrysCodex.onPlayerEndTurn` → `CodexAction`)
before power `atEndOfTurn` (Combust). The first Codex frame therefore
still has pre-Combust HP and enemy block (FIDL01727 step 786). Combust
is applied when end-turn resumes after that offer.

## Model (ordinary-first)

- **Single-offer (default / permanent tips):** first CHOOSE inserts immediately
  into the draw pile; resume continues `callEndOfTurnActions`
  (`triggerOnEndOfTurnForPlayingCard` exhausts unplayed ethereal, then
  DiscardAtEndOfTurnAction) before monster→draw (FIDL00108 Ghostly Armor).
- **Two-step (FIDL00451 / FIDL01772):** first CHOOSE still inserts into the
  draw pile (the 607 frame has the extra card). The following `END` is not a
  full resume: it discards the remaining hand and opens a **second** offer
  before monsters act. Detect that `END` fail-closed when the observed frame
  is `CARD_REWARD` with an empty hand (`deferred_nilrys_second_offer_on_end_candidate`).
  - stage 1: first offer open
  - CHOOSE inserts and closes the first offer; stage stays 1 until the next
    command is classified
  - END matching the second-offer frame: discard hand, open second offer,
    stage → 3
  - CHOOSE/SKIP at stage 3: insert only a CHOOSE pick, then `end_player_turn`
    (duplicate captured monster queue + two rolls). When the first offer
    already inserted, a later CHOOSE can close without a second insert
    (`deferred_nilrys_second_choice_without_insert_candidate`; FIDL01486
    CHOOSE 610).
  - PLAY after a stage-1 SKIP (offer already closed): SuperFastMode may
    discard that hand (swallowing the PLAY) and publish the next turn
    (`deferred_nilrys_leftover_end_instead_of_play_candidate`). Rejected PLAY
    still uses discard-only leftover settlement plus STATE polls.
  - PLAY after a completed leftover close can still be a queued EndTurn that
    opens the next first Codex with the new hand held
    (`deferred_nilrys_play_opens_next_first_codex_candidate`; FIDL01486
    PLAY 621).

## Step 508 residual (open)

Sim after stage-3 finalize (one Double Strike 6×2 through block 6):

- HP 9888 (−6), times_damaged path → 1 Runic Cube draw, hand size 6
- next intent re-rolls Double Strike (`move_history` `[2, 2]`)

Real step 508:

- HP 9876 (−18), `times_damaged` 0→**3**, hand size **8** (base 5 + 3 RC)
- next intent Suck (`move_id` 3), `last_move_id=2`, `second_last_move_id=2`
  ⇒ history pattern `[2, 2, 3]` (two Double Strike executions then Suck)
- mon plated block 12 (consistent with end-of-turn plated after the sequence)

Diagnostic: two sequential DS turns with the trace’s post-first-roll next=DS
would yield HP −18, 3 RC draws, hand 8, history `[2,2,3]`, final Suck — full
match.

The second `END` is a second `EndTurnAction` while the first is still queued
behind Codex (SuperFastMode leftover). Both actions enqueue a
`MonsterQueueItem` before either `RollMoveAction` — the same multiplicity as
Time Warp (`design_time_warp_duplicate_queue.md`). Stage-3 close therefore
executes the captured intent twice, then two `rollMove`s, then one next-player
draw. Each EndTurnAction still runs `atEndOfTurn`, so Combust ticks at the
stage-2 END and again on the stage-3 close (FIDL01727). Single-offer tips never
reach stage 3.

Witnesses: FIDL01772 (two Snecko Bites → Tail), FIDL01727 (two SG slams + two
Sentry bolts; leftover Book 5-hit resume at STATE 867), FIDL00451 (two
Shelled Parasite Double Strikes).

Book of Stabbing: the first multi-stab two-step of a fight executes
captured 2-hit then 3-hit (FIDL01727 step 852). Later two-steps default
to captured N+N (step 880: two 6-hits). When the observed frame matches
live `stabCount` after the first takeTurn, the second queue item uses
N+1 hits without incrementing `StabCount` (`getMove` still owns that
counter). Step 887 is 7+8.

Each `MonsterQueueItem` runs every living monster, then the next item
does the same (FIDL01597: Mad Gremlin 4+2, Rally, then 4+5). Nesting the
duplicate inside each actor would land both hits before Encourage.

The leftover first `EndTurnAction` can also run `MonsterStartTurnAction`
`loseBlock` while the second Codex offer is still open (FIDL01597 step
460: Rally block gone, takeTurn has not run). Stage-2 END therefore
clears living monster block without executing the queue. SuperFastMode
may also publish only one `MonsterQueueItem` on the stage-3 close
when that frame matches. FIDL01486 still uses the duplicate queue once
leftover plated lands; SuperFastMode can hold Byrd's Caw display
(skip StrengthSelf RollMoves) while Chosen consumes both leftover rolls,
or publish after Byrd's first leftover roll while Chosen still shows Drain
(FIDL01486 SKIP 468).

Closing the first Codex offer continues `callEndOfTurnActions` card
autoplays (Regret/Burn) without Combust or the bulk hand discard
(FIDL01597 CHOOSE 470: Regret 4 HP, other cards stay). The same resume
window can apply Plated Armor / Metallicize automatic block while the
pre-discard hand is still held (FIDL01486 CHOOSE 461: block 5→9 from
Thread and Needle, Evolve not yet in the draw pile). A later first-offer
CHOOSE in the same fight can insert the pick *and* grant that block
(FIDL01486 CHOOSE 466: Iron Wave enters the draw pile, block 0→4).
An extra leftover EndTurn can open another Codex while the hand is still
held (FIDL01486 END 475). Stage-2 END must not grant plated again when
that frame discards. The leftover EndTurn's matching `atEndOfTurn` lands
on the final Codex close before the duplicate MonsterQueue.

Unit: `nilry_two_step_second_offer_runs_two_snecko_bites_then_tail`,
`nilry_two_step_gremlin_leader_rally_applies_between_duplicate_hits`,
`nilry_two_step_second_choice_ticks_leftover_plated_before_duplicate_queue`.

## Non-goals

- Do not force two-step when immediate insert matches (regresses permanent tips).
- Do not insert both first and second picks (multiset is +1 only).
- Do not run two full player-turn cycles (second Combust / second draw) on
  this path; only the two captured monster queue items plus two rolls.
