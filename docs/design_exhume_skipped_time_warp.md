# Skipped Exhume retrieval still increments Time Warp

## Source behavior

`Exhume.use()` opens the exhaust grid. `UseCardAction` completes when the
grid closes, and `TimeWarpPower.onAfterUseCard` increments then.

CommunicationMod can publish a stable post-CHOOSE frame after Exhume
settles (source exhausted, Dark Embrace may draw) but before the selected
card leaves exhaust. The verifier accepts that with
`confirm_exhume_select_skipped_return`.

That skip discarded `exhaust_select.pending_actions`, including
`ApplyDeferredTimeWarpCardPlay`. FIDL01586 Havoc→Exhume CHOOSE 1588 and
FIDL01803 Havoc→Exhume CHOOSE 1696 therefore never incremented Time Warp.
Later 12th-card PLAY (Defend / Heavy Blade) did not force end-turn.

## Decision

Skipped Exhume return still drains `pending_actions` and settles Time Warp
the same way a normal Exhume CHOOSE does. The selected exhaust card stays
in exhaust.

## Non-goals

- Do not retrieve the selected card on the skipped path.
- Do not change ordinary Exhume CONFIRM.
