# CommunicationMod Trace Notes

## trace-2026-06-25T00-44-15-558Z.jsonl

Status: structurally valid but not pristine verification evidence.

This trace was collected during a successful live run, but it has a known manual-input and bridge-sync contamination window near the Act 3 boss fight.

Known issues:

- Step 806 records an `END` transition that the user reported was clicked directly in the game instead of through the trace UI. The action record has been annotated with `source: "manual_game_click"`.
- Starting around step 807, the trace UI appeared one action behind the live game.
- Several gameplay commands were followed by stale post-action states. A subsequent explicit `state` command then captured the actual updated live state.
- Error records around steps 814 and 815 came from stale UI indices/targets after the desync.
- Later collection continued with the workaround: take an action through the UI, then immediately send `state` before trusting hand indices, monster HP, energy, or block.

Observed examples:

- Step 818 `END` was followed by a stale turn-5 state. Step 819 `state` then captured the real turn-6 state.
- Step 820 `PLAY 4 2` was followed by a stale turn-6 hand/energy state. Step 821 `state` then captured the updated energy and hand.
- Step 824 `end` was followed by a stale turn-6 state. Step 825 `state` then captured the real turn-7 state.
- Step 826 `PLAY 9` was followed by a stale state. Step 828 `state`, after another play, captured an updated hand.

How to use this trace later:

- It is usable for broad coverage, import robustness, and as a source of real game states.
- It should not be used as clean end-to-end action parity evidence across the contaminated boss-fight window.
- Treat explicit `state` records after gameplay commands as re-anchor points for the true live state.
- A clean prefix can be extracted before the manual END/desync window.
- A repaired/split suffix may be possible by keeping only anchored state segments and documenting skipped stale/error records.
- Any verifier using this trace should either skip the contaminated window or classify it as expected manual/desync contamination rather than simulator mismatch.

Verifier feasibility check on 2026-06-26:

- `node tools\communication\trace_tools.js validate verification\corpus\communication_mod\trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl` passes structural validation (`actions=548`, `max_floor=37`, `elite_rooms=5`, `boss_rooms=4`, terminal map floor 37).
- `cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl` verifies only the `START IRONCLAD 0 MANUAL01` bootstrap action and then fails at Neow with `unexpected_diffs=1`, first boundary `$.actions[step=3].command`, category `unexpected_seed_start_command`.
- The root blocker is that MANUAL01's Neow option set/choice is outside the seed-start harness's captured Neow branches. The trace therefore cannot currently support a no-observed-restoration parity claim for later shop/reward/deck/colorless-card observations, even in the clean prefix.
- The raw full trace remains useful as broad real-state coverage and for future anchored suffix experiments, but not as clean end-to-end trace parity evidence.

Recommended next trace shape:

- For M29, collect a fresh single-run Sentries/elite trace on a verifier-supported seed-start branch and continue one action past the final target reward screen with `PROCEED`.
- For 32C targeted card claims, collect short traces where the target card appears after a currently supported Neow/map/combat prefix, preferably one target surface per trace.
- Drive every gameplay action through the trace UI and stop collection immediately if the UI appears one action behind the live game; do not repair a targeted parity trace with manual clicks or post-action `state` re-anchors.
