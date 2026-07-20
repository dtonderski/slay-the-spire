# Observable monster power verification

## Scope

The strict verifier previously removed monster Strength, Ritual, and Vulnerable
from every combat comparison. This made stable, gameplay-relevant divergences
invisible.

## Contract

- Compare Strength, Ritual, and Vulnerable for every living monster.
- Project Strength and Ritual as independent powers; Ritual is not subtracted
  from accumulated Strength.
- Omit these powers for dead monsters. CommunicationMod exposes dead-monster
  powers inconsistently between lethal and settling frames, and those values no
  longer affect gameplay.
- Keep intent outside this change. Intent comparison has distinct naming and
  timing concerns and will be enabled in a separate slice.

## Mechanic correction

Donu and Deca start with 2 Artifact at baseline ascension and 3 at Ascension 19.
The previous baseline of zero caused start-of-combat debuffs such as Red Mask
and Bag of Marbles to appear in simulated state even though the observed game
consumed the two Artifact charges.

Bag of Marbles must use the ordinary monster-debuff path. Directly incrementing
Vulnerable bypasses Artifact; Champion Belt's follow-up Weak must occur only
when Vulnerable was actually applied.

Darkling death clears its temporary power state. On reincarnation, persistent
combat-wide effects must be restored; currently this includes the +1 Strength
from Philosopher's Stone.

The verifier must also derive Act 1 and Act 3 boss identities from the run seed
and supplied unlock state before applying each map transition. Applying that
derivation only to the final report leaves the replay using default boss state
and can make a correctly named projection conceal the wrong encounter.

## Verification

Focused tests cover living/dead normalization, independent Strength/Ritual
projection, and both Donu/Deca Artifact thresholds. Permanent and fidelity
corpora must remain strict and green.
