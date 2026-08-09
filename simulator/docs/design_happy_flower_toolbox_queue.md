# Happy Flower / Toolbox opening queue

Target source queues the opening combat actions in this order:

1. base energy;
2. Toolbox's colorless-card choice;
3. opening hand draw and combat-start actions;
4. Happy Flower's `GainEnergyAction` from `atTurnStart`.

`HappyFlower.atTurnStart` updates its counter synchronously, but its energy is an
action queued behind Toolbox. Therefore, while Toolbox's choice screen is open,
the counter is already reset to zero but the player still has only base energy.

The simulator will retain the counter update and record the energy as a pending
start-of-turn relic action when first-turn Toolbox blocks the queue. Choosing the
Toolbox card settles that pending energy. The same queue boundary now parks the
opening hand draw and start-of-combat block actions: while the Toolbox choice is
visible, the authoritative piles remain fully in the draw pile and Anchor block
has not applied; the choice transition settles those queued actions before the
next player boundary. Later turns and combats without Toolbox keep the current
immediate behavior because their stable observations occur after the action queue
drains.

Evidence: target `AbstractRoom` opening-combat sequence, `Toolbox.atBattleStartPreDraw`,
`Anchor.atBattleStart`, and `HappyFlower.atTurnStart` in the installed target JAR;
CommunicationMod state step 514 / choice step 515 in
`FIDL01322-p1322-2026-08-08T17-11-10-444Z-2239647.jsonl`; and live
CommunicationMod trace `session-1203.jsonl`, step 7569. The trace's later card
retrieval frame is intentionally outside this slice; no skipped retrieval is
inferred from observation timing.
