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

## Model (ordinary-first)

- **Single-offer (default / permanent tips):** first CHOOSE inserts immediately
  into the draw pile; next combat command resumes discard→monster→draw.
- **Two-step (FIDL00451):** when immediate insert does not match the post-frame:
  - stage 1: first offer open
  - CHOOSE closes offer **without** parking/inserting; stage → 2
  - END at stage 2: discard hand, open second offer, stage → 3
  - CHOOSE at stage 3: insert **only** this pick, `end_player_turn` resume

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
match. STS Nilry opens **once** per end turn and runs **one** monster phase;
permanent single-offer tips stay green under that model. Double monster turn is
therefore **not** wired as generic behavior without source for a second
`MonsterQueueItem` / second `takeTurn` on this path.

Unit: `nilry_two_step_second_choose_runs_monster_and_changes_intent`.

## Non-goals

- Do not force two-step when immediate insert matches (regresses permanent tips).
- Do not insert both first and second picks (multiset is +1 only).
- Do not double `run_monster_turn` to chase FIDL00451 without authoritative
  queue evidence.
