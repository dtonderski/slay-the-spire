# Writhing Mass Parasite settlement

Writhing Mass's Mega Debuff queues `AddCardToDeckAction(Parasite)` before its
later `RollMoveAction`. `AddCardToDeckAction.update()` constructs
`ShowCardAndObtainEffect`, whose constructor immediately calls
`CardHelper.obtain`; the master deck mutation and card-obtain relic callbacks
therefore settle before the next command-ready boundary.

The combat core records execution with
`writhing_mass_mega_debuff_triggered`. The run wrapper transfers that marker to
`pending_combat_obtain_cards` and drains the typed obtain action in the same
accepted transition. This ordering is shared by ordinary `END` and by an
end-turn queue resumed after an interaction such as Nilry's Codex.

Nilry's `onPlayerEndTurn` opens `CodexAction` before the monster turn, so the
opening card reward correctly has no new Parasite. Closing or skipping the
Codex offer resumes the queued end turn; if Mega Debuff executes, the common
post-combat-transition settlement helper obtains exactly one Parasite before
publishing the resulting state. Omamori and card-obtain relics remain part of
that normal obtain path.

Observed deck contents never select, flush, or synthesize this transition. A
trace that publishes an intermediate queue frame must be represented by its
source-backed queued action state or rejected as invalid input; replay does not
copy the observed Parasite or condition command execution on a post-state
match.
