#!/usr/bin/env node

const assert = require("assert");
const { summarize, validate } = require("./trace_tools");

function state(step, gameState) {
  return { type: "state", step, message: { game_state: gameState } };
}

function action(step, command) {
  return { type: "action", step, command };
}

function testCoverageSummaryForEliteRewardPrefix() {
  const records = [
    state(0, { floor: 0, screen_type: "MAIN_MENU" }),
    action(1, "START IRONCLAD 0 M290001"),
    state(1, { floor: 0, screen_type: "NEOW", seed: "M290001", act_boss: "The Guardian" }),
    action(2, "CHOOSE 0"),
    state(2, { floor: 7, screen_type: "COMBAT", room_type: "MonsterRoomElite", room_phase: "COMBAT" }),
    action(3, "END"),
    state(3, {
      floor: 7,
      screen_type: "COMBAT_REWARD",
      room_type: "MonsterRoomElite",
      room_phase: "COMPLETE",
      screen_state: { rewards: [{ reward_type: "GOLD" }, { reward_type: "CARD" }] },
    }),
  ];

  const result = validate(records);
  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.summary.actions, 3);
  assert.strictEqual(result.summary.max_floor, 7);
  assert.strictEqual(result.summary.elite_rooms, 1);
  assert.strictEqual(result.summary.boss_rooms, 0);
  assert.strictEqual(result.summary.terminal.kind, "reward_screen");
  assert.deepStrictEqual(result.summary.terminal.rewards, ["GOLD", "CARD"]);
  assert.strictEqual(result.summary.coverage.reached_elite, true);
  assert.strictEqual(result.summary.coverage.reached_boss, false);
  assert.strictEqual(result.summary.coverage.ended_cleanly, true);
}

function testMissingActionStillFailsValidation() {
  const result = validate([state(0, { floor: 0 }), action(1, "END")]);
  assert.strictEqual(result.ok, false);
  assert.deepStrictEqual(result.missing, [{ line: 2, step: 1, command: "END" }]);
}

function testDeathTerminal() {
  const summary = summarize([state(1, { floor: 1, screen_type: "GAME_OVER", current_hp: 0 })]);
  assert.strictEqual(summary.deaths, 1);
  assert.strictEqual(summary.terminal.kind, "death");
  assert.strictEqual(summary.coverage.has_death, true);
}

testCoverageSummaryForEliteRewardPrefix();
testMissingActionStillFailsValidation();
testDeathTerminal();

console.log("trace_tools tests passed");
