# Leftover PLAY rebinds the unused command after it resolves

## Source behavior

Writhing Mass Mega Debuff can publish Parasite on a PLAY that Java rejects
(Ceramic Fish gold, deck +1, combat hand unchanged). That PLAY stays on the
action queue. The next command resolves the leftover play; the unused
command is then queued against the hand *after* that leftover resolves.

FIDL01617: PLAY 1 Dropkick is rejected on the Parasite frame. PLAY 2 0 is
captured while the leftover Dropkick still occupies slot 1. After Dropkick
resolves, slot 2 is True Grit. Binding PLAY 2 against the pre-leftover hand
selects Heavy Blade+ and the following PLAY 1 plays it (block 0, energy 1).
Real plays leftover True Grit (block 7, energy 0).

## Implementation

When a leftover play candidate is accepted, rebind the current command with
`direct_decision` on the post-leftover state. Do not keep the pre-leftover
card id.

## Non-goals

- Do not change Madness remaining-hand sampling.
- Do not skip True Grit's random exhaust; it must still use cardRandomRng.
