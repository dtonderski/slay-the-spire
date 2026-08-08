# Havoc empty-draw source settlement

## Decision

`havoc_queue` normally plays the forced top card before settling Havoc when the
draw pile is empty. For a non-exhausting Havoc with an empty draw pile, it may
settle Havoc first only when the Java-equivalent reshuffle preview (including
the source in discard) makes the forced card Headbutt/Headbutt+. This preserves
Headbutt's discard-to-draw selection of the played source. The source-included
preview is disabled while Dark Embrace is active.

## Evidence

- The source-backed transition path models `DarkEmbracePower.onExhaust` as a
  deferred `DrawCards` follow-up (`combat/transition.rs`,
  `dark_embrace_draw_follow_up`), so an exhausted forced card must resolve
  before a source-settlement-triggered draw can consume the reshuffled pile.
- `transition.rs::havoc_places_source_in_discard_before_headbutt_can_return_it_to_draw`
  isolates the ordinary Headbutt case where Havoc must be visible to the
  discard selection and ends with Havoc on the draw pile.
- FIDL00276 step 570 is the counterexample: Havoc+ has an empty draw pile,
  Dark Embrace, and Headbutt in discard; the observed next frame exhausts
  Body Slam, then draws from the pile, with Havoc in discard. Including Havoc
  in the refill changes the forced card and the subsequent draw order.
- FIDL00384 step 1003 is the ordinary empty-draw Headbutt+ witness and
  verifies with the source-included preview disabled because the previewed
  forced card is not Headbutt when Havoc is included.

The predicate depends only on card/action semantics and authoritative combat
powers. It does not inspect trace identity, observed state, seed, or RNG
counters; the preview uses a cloned state and consumes no live RNG.
