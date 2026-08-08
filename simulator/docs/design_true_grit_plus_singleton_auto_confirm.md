# True Grit+ singleton exhaust auto-confirm

## Evidence (FIDL00394 step 1239)

Havoc+ force-plays True Grit+ while the only other hand card is Strike_R+.
CommunicationMod publishes a settled post-frame (`screen_type=NONE`, empty hand,
Strike_R+ and True Grit+ both in exhaust) without HAND_SELECT / CONFIRM.

True Grit+ always opens `ExhaustSelect` even with one candidate (unlike base
True Grit's single-card auto-exhaust path). SuperFastMode can auto-pick the
singleton and omit the grid frames.

## Fix

After PLAY, if the sim holds `TrueGritExhaustOne` with exactly one selectable
hand card and the observation is not on a select screen, try:

1. Ordinary `choose` + `confirm_exhaust_select` (exhaust selection + source)
2. Else force-play skipped-retrieval (`confirm_true_grit_select_skipped_retrieval`)

Accept the first candidate whose combat subset matches the observation.

## Non-goals

- Do not auto-confirm multi-card True Grit+ grids (FIDL00238 / FIDL00253 need
  explicit CHOOSE/CONFIRM or skipped-retrieval on CONFIRM).
