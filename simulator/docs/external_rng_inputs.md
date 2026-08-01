# External RNG Inputs

Slay the Spire normally uses named, seed-derived RNG streams. A small number of
gameplay call sites instead use libGDX's process-global `MathUtils.random`.
That stream is advanced by UI and other process activity, so its state cannot be
derived faithfully from a run seed.

Strict replay treats each such gameplay draw as explicit trace input:

- CommunicationMod captures the two `RandomXS128` state words immediately
  before the target draw.
- State words are encoded as fixed-width hexadecimal strings so the JavaScript
  trace bridge cannot lose 64-bit precision.
- Captures are ordered and associated with the command that caused them.
- Core snapshots serialize pending external draws. Mechanics consume a
  purpose-matched draw exactly once and fail closed when it is absent or
  mismatched.
- Replaying a captured state never advances any named, seed-derived RNG stream.

The first supported call site is The Courier's colored-card replacement.
`cardRng` still rolls replacement rarity, while the captured `MathUtils` state
selects an identity from the target-sorted card-type pool. Colorless Courier
replacement remains fully seeded: `merchantRng` rolls rarity and `cardRng`
selects identity.

Legacy traces without call-time metadata are not identity evidence. They remain
valid up to the first external draw, where strict replay reports
`missing_external_rng` instead of substituting an observed card or a seeded
approximation.
