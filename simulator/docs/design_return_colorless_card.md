# returnColorlessCard (Knowing Skull / Match and Keep)

## Target behavior

`AbstractDungeon.returnColorlessCard(CardRarity)` does **not** sample
`cardRng` over a rarity-filtered list. Bytecode (`AbstractDungeon.returnColorlessCard`):

1. Take the live `colorlessCardPool.group` list.
2. `Collections.shuffle(list, new java.util.Random(shuffleRng.randomLong()))`.
3. Iterate the shuffled list; return the first card whose rarity matches.
4. If the requested rarity is `RARE` and none matched, return the first
   `UNCOMMON` in that same shuffled order.
5. Otherwise fall back to `new SwiftStrike()`.

The shuffle mutates the global pool order, so later
`returnColorlessCard` / Match-and-Keep colorless rolls must continue from the
mutated order and consume further `shuffleRng.randomLong()` seeds.

## Simulator mapping

- Pool identity: `colorless_match_and_keep_pool()` (CardLibrary/`colorlessCardPool`
  order, including Bandage Up).
- Stream: `RunRngStream::Shuffle`.
- Helper: `return_colorless_card_from_pool` in `shop_pool.rs`.
- Persistence: `RunState.colorless_card_pool` (lazy-init when empty).

## Call sites

- Knowing Skull Success (`knowing_skull_gain_random_colorless`)
- Match and Keep colorless slot (non-A15)

Shop / Neow / combat rewards that use `cardRng` + rarity-filtered pools remain on
`random_colorless_from_pool`; that is a different target helper.

## Knowing Skull obtain timing

Success queues `ShowCardAndObtainEffect` (`queue_pending_obtain_card`). The next
Knowing Skull option (including Leave) flushes pending obtains at the start of
the update, matching rapid multi-Success captures where the last card can lag
into the Leave/MAP frame (FIDL00445).
