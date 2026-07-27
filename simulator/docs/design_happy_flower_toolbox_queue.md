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
Toolbox card settles that pending energy. Later turns and combats without Toolbox
keep the current immediate behavior because their stable observations occur after
the action queue drains.

Evidence: target `AbstractRoom` opening-combat sequence, `Toolbox.atBattleStartPreDraw`,
and `HappyFlower.atTurnStart` in the locally decompiled target game sources; live
CommunicationMod trace `session-1203.jsonl`, step 7569.
