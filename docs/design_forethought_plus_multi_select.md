# Forethought+ multi-select (FIDL00269)

## Observations

1. **CHOOSE indexing**: CommunicationMod removes each selected card from
   `choice_list`. Repeated `CHOOSE 1` must map to the next *unselected* hand
   slot, not toggle the same authoritative index.

2. **Skipped retrieval**: Some CONFIRM frames leave the draw pile unchanged
   while selected cards leave the hand (PutOnDeck completed before CONFIRM).
   Selected cards re-enter via end-turn discard limbo
   (`pending_hidden_hand_card_until_end_turn`), same family as single-card
   Warcry/Forethought skipped retrieval.

## Rules

- `hand_select_ui_to_hand_index` for `ForethoughtPutAnyOnDraw` excludes already
  selected indices.
- Ordinary multi CONFIRM still `insert(0)` (draw bottom) when that matches.
- Verifier ordinary-first; else `confirm_forethought_multi_select_skipped_retrieval`.

## API

- `confirm_forethought_multi_select_skipped_retrieval`
