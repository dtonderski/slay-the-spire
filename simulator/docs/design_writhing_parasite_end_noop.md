# Writhing Mass Parasite / Implant END lag (FIDL00260)

## Observation

After Implant, CommunicationMod can lag across multiple frames:

1. **Implant EndTurn** (e.g. step 1100): turn advances, new hand drawn, but
   Parasite may be missing from the published deck for one frame.
2. **Publish / turn-end lag END** (e.g. step 1101): same turn's hand still
   visible while Parasite + Ceramic Fish gold appear; this END is still a real
   turn boundary (turn N → N+1). Empty-hand STATE polls then the next hand
   follow.
3. Treating 1101 as a **no-op** left the sim on turn N while real advanced,
   desyncing the next PLAY (FIDL00260 step 1107).

## Rules

1. **Implant combat lag** (`seed_start_writhing_mass_implant_end_turn_lag`):
   Implant flipped `has_siphoned`, observation still matches pre-EndTurn combat
   (ignore deck/gold), sim EndTurn diverges → apply EndTurn, keep piles, defer
   deck if Parasite not yet observed.
2. **Parasite publish lag** (`seed_start_end_turn_writhing_parasite_publish_lag`):
   Observation still matches pre-EndTurn combat with Parasite/gold publishing →
   **apply** EndTurn (`next`), do not no-op, do not rebind piles to the lagged
   hand.
3. Never `bind_combat_piles_to_source_order` onto a post-EndTurn sim using a
   pre-discard CM hand after Implant.

## Residual

None for FIDL00260 (promoted complete_pass).
