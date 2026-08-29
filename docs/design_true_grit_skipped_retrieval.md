# True Grit ExhaustAction skipped-retrieval

## Evidence

- FIDL00238 steps 832–834 (Havoc → True Grit+, select Power Through+):
  CONFIRM exhausts both Power Through+ and True Grit+, Feel No Pain fires twice
  (`block 12 → 18`). Ordinary retrieval.
- FIDL00253 steps 1327–1329 (Havoc → True Grit+, select Evolve from Dead Branch
  context): CONFIRM exhausts only True Grit+; selection is absent from every
  serialized pile and re-enters discard on END; Feel No Pain fires once.

## Implementation

- Core default: `confirm_true_grit_select` always exhausts the selection (and the
  force-played True Grit source when parked on the decision).
- Core alternate: `confirm_true_grit_select_skipped_retrieval` parks the selection
  in `pending_hidden_hand_card_until_end_turn` and exhausts only True Grit via
  `CardExhausted` (Feel No Pain / Dark Embrace / Dead Branch).
- Verifier: `seed_start_true_grit_skipped_retrieval_state` is eligible only when
  the exhaust-select decision already holds `source_card` (force-play). Accept
  only when the skipped candidate matches the observed combat subset.

## Non-goals

- Do not force-hide every force-play True Grit selection (regresses FIDL00238).
- Do not mark Power Through exhaust via card keywords alone (definition remains
  `CARD_KEYWORDS_NONE` in content defs despite wiki exhaust).
