# Observable monster intent verification

## Scope

Strict combat comparison previously removed every monster intent. Stable,
gameplay-relevant intent differences must instead be projected into
CommunicationMod's intent taxonomy and compared for living monsters.

## Visibility contract

- Compare intent for each living monster on stable combat frames.
- Omit intent for dead monsters because retained dead slots have no actionable
  intent.
- CommunicationMod can report the target's stable `Intent.DEBUG` category even
  when `move_id` identifies the selected move. In that case the category itself
  has no semantic equivalent to compare, so compare the source-backed move byte
  and every other stable monster field instead. This is an explicit visibility
  contract, not a transient-frame pass or permission to drop the whole monster.
- Reduced/manual observations that omit `move_id` do not gain a synthetic zero
  byte at projection time. The byte is compared whenever the observation
  exposes it; absence remains an explicit observation-visibility limitation.
- Runic Dome hides both intent category and move byte. The observed projector
  derives that visibility from observed relics, while the simulated projector
  derives it independently from simulator relic state; neither projector uses
  the other side to decide what the run contains.
- After player HP reaches zero, an executing lethal-damage frame may retain a
  stale or already-prepared enemy intent, but no later player decision can
  observe or act on it. Terminal projections omit only intent and move byte;
  player HP, living-monster powers, and other stable fields remain strict.
- Use CommunicationMod categories such as `DEFEND`, `DEFEND_BUFF`,
  `ATTACK_DEFEND`, and `STRONG_DEBUFF`; do not collapse them into broader labels.
- Preserve source-specific categories where the same typed effect is used as an
  implementation marker. In particular, Byrd move byte 2 is target
  `Intent.UNKNOWN` even though core represents its zero-strength state change
  with `StrengthSelf { amount: 0 }`. Spiker move byte 2 is target `Intent.BUFF`
  because it raises Thorns; its zero-block typed representation must not be
  projected as `DEFEND_BUFF`.

## Gremlin Wizard cycle

Target `GremlinWizard` bytecode initializes `currentCharge` to 1. Charge turns
increment it and select Attack when it reaches 3. Attacking resets it to 0. At
Ascension 0 this yields attacks after executed-move counts 2, 6, 10, and so on;
at Ascension 17 the first Attack repeats without returning to Charge. Charge is
shown as `UNKNOWN`, not `DEFEND`.

## Locked opening intents

The Jaw Worm Horde spawn helper precomputes each opening intent from the combat
AI seed and locks it against generic opening rerolls. Those locked intents still
represent one `aiRng.random(99)` draw per Jaw Worm. Combat entry must advance
the authoritative AI stream for each locked horde member so the first end-turn
roll starts after, rather than reusing, the opening draws.

## Reptomancer move history

Target `Reptomancer.getMove` prevents a third consecutive summon with
`lastTwoMoves(2)`. Its Snake Strike is move byte 1 and is displayed as
`ATTACK_DEBUFF`; the simulator represents it as
`AttackMultipleApplyPlayerWeak`. That typed representation must still be
recorded as byte 1. Omitting it turns the real sequence `2, 1, 2` into `2, 2`
and incorrectly forces Snake Strike after the second summon.

The same move-byte authority must cover deterministic cycles such as Cultist
and Hexaghost, plus scripted encounters such as the Masked Bandits.
Verification uses these bytes when CommunicationMod exposes a `DEBUG`/`UNKNOWN`
category, while core history uses them for repeat guards; the mapping is
therefore shared rather than reimplemented in the verifier.

## Slime move tables

Small Acid Slime uses move byte 2 for Lick, while medium and large Acid Slimes
use byte 4. A recorded byte 2 therefore cannot be used to infer that an Acid
Slime is medium. Intent preparation and move-byte projection must prefer the
explicit `SlimeSize` carried by encounter construction and split mechanics;
HP/intent inference is only a compatibility fallback for older unchecked
states that lack the discriminator.

## Damage-triggered intent changes

Target `Lagavulin.damage` changes a sleeping Lagavulin to move byte 4 (`STUN`)
whenever HP decreases, independent of whether the damage came from a card.
Potion damage therefore runs the same wake hook as card damage. This is an
authoritative core transition; the verifier only observes the resulting intent.

## Verification

Focused projector tests cover the distinct intent categories and the `DEBUG`
move-byte visibility contract. The Wizard unit test pins the source-backed
cycle. Permanent and fidelity corpora must remain strict and green.
