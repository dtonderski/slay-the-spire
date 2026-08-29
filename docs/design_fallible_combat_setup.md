# Fallible Combat Setup

## Problem

The public deck-to-combat setup helpers classified missing card definitions as
ordinary cards. An unknown card was treated as non-Innate, was shuffled into a
plausible pile, and was skipped by Snecko Eye. The starter-deck classifier also
reported an unknown deck as merely non-starter. Because combat setup consumes
shuffle and card-random RNG, discovering bad content later could additionally
leave caller-owned streams advanced.

## Decision

Card-definition lookup is a fallible precondition of opening-pile generation
and starter-deck classification. Unknown content returns `UnknownContent`.
Opening-pile generation validates the complete deck before either RNG stream is
used, then preserves the existing target shuffle, Innate/bottled ordering, and
Snecko Eye draws exactly. Core run and event combat entry propagate the error;
the verifier's trace regression requires its imported deck to be valid.

The low-level deck-order copy helper remains infallible because it neither
interprets content nor consumes RNG.

## Verification

Regressions require unknown content to fail before shuffle or card-random RNG
advances, and require the direct Innate and starter-deck classifiers to reject
the same input. Existing seed-start traces continue to pin opening hand and draw
pile ordering.
