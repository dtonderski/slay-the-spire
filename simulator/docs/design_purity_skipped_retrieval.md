# Purity ExhaustAction skipped-retrieval

## Evidence

FIDL00405 steps 1188–1193:

- Purity opens exhaust select; player chooses Sword Boomerang, Bash, Reckless Charge.
- On CONFIRM: only `Purity` is in exhaust, Feel No Pain fires once (`block 8 → 11`),
  hand keeps the unselected Wounds, and the three chosen cards are absent from
  every serialized pile.
- On the following END: the three chosen cards re-enter discard (with ethereal
  Wounds), matching HandCardSelectScreen leftover `selectedCards` flush.

FIDL00238 force-play True Grit+ has the dual pattern (selection exhausts on some
CONFIRMs, stays hidden on others). Ordinary Purity retrieval still exhausts the
selection immediately and remains the default combat path.

## Implementation

- Core: `confirm_purity_select` keeps full retrieval (exhaust selection + Purity).
  Each exhaust applies relic `onExhaust` (Dead Branch) before power `onExhaust`
  (Dark Embrace draw), matching `AbstractPlayer.onExhaust` (FIDL01373 empty
  select: generated Pommel then drawn Dark Embrace). Hooks run in place so
  multi-select CONFIRM stays a valid unique-id state (FIDL01582).
- Core: `confirm_purity_select_skipped_retrieval` parks the selection in
  `pending_hidden_hand_card_until_end_turn` and exhausts only Purity (via
  `CardExhausted` so Feel No Pain / Dark Embrace / Dead Branch stay ordered).
- Verifier: `seed_start_purity_skipped_retrieval_state` builds the skipped
  candidate and accepts it only when:
  1. the ordinary CONFIRM settlement does **not** match the observed frame, and
  2. the skipped candidate does match.

Combat subsets omit exhaust pile contents, so without (1) a no-FNP Purity
exhaust would falsely match skipped-retrieval from hand/discard alone and dump
the selection into discard on END, desyncing later shuffle order.

## Unceasing Top

If skipped CONFIRM empties the published hand, Unceasing Top draws immediately
(FIDL01681: Warcry from the top of draw). The parked selection still re-enters
discard only on the next non-empty END.

If combat ends on a mid-turn PLAY before that END, the leftover `selectedCards`
stay on the singleton screen. The next combat's first non-empty END can publish
every parked card beside the live master-deck copies (FIDL01582: Dropkick and
two Strikes). The verifier remints transient instance ids and settles them
through the ordinary end-turn pending-hidden path.

## Non-goals

- Do not change ordinary Purity retrieval semantics.
- Do not weaken combat subset comparison or invent seed-specific branches.
