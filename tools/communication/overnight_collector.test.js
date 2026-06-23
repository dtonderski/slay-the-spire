#!/usr/bin/env node

const assert = require("assert");
const policy = require("./overnight_collector");

function baseSummary(overrides = {}) {
  return {
    step: 1,
    in_game: true,
    ready_for_command: true,
    available_commands: [],
    screen_type: "NONE",
    choices: [],
    ...overrides,
  };
}

function test(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

test("full potion belt reward proceeds instead of choosing potion", () => {
  const summary = baseSummary({
    screen_type: "COMBAT_REWARD",
    available_commands: ["choose", "proceed", "state"],
    choices: ["potion"],
    potions: [
      { name: "Fire Potion" },
      { name: "Snecko Oil" },
      { name: "Blood Potion" },
    ],
  });
  assert.strictEqual(policy.rewardCommand(summary), "PROCEED");
});

test("empty potion slot reward chooses potion", () => {
  const summary = baseSummary({
    screen_type: "COMBAT_REWARD",
    available_commands: ["choose", "proceed", "state"],
    choices: ["potion"],
    potions: [
      { name: "Fire Potion" },
      { name: "Potion Slot" },
      { name: "Blood Potion" },
    ],
  });
  assert.strictEqual(policy.rewardCommand(summary), "CHOOSE 0");
});

test("card reward fallback picks a card instead of skip when choose is available", () => {
  const summary = baseSummary({
    screen_type: "CARD_REWARD",
    available_commands: ["choose", "skip", "state"],
    choices: ["clash", "sentinel", "rupture"],
  });
  assert.strictEqual(policy.cardRewardCommand(summary), "CHOOSE 0");
});

test("choice labels support structured bridge choices", () => {
  assert.strictEqual(policy.choiceLabel({ label: "Elite" }), "Elite");
  assert.strictEqual(policy.choiceLabel({ symbol: "M" }), "M");
  const summary = baseSummary({ choices: [{ label: "Event" }, { symbol: "E" }] });
  assert.strictEqual(policy.choiceIndex(summary, [/^e$/, /elite/]), 1);
});

test("map command scores visible choices toward elites and fights", () => {
  const eliteSummary = baseSummary({
    screen_type: "MAP",
    available_commands: ["choose", "state"],
    choices: ["?", "M", "E"],
  });
  assert.strictEqual(policy.mapCommand(eliteSummary), "CHOOSE 2");

  const fightSummary = baseSummary({
    screen_type: "MAP",
    available_commands: ["choose", "state"],
    choices: ["?", "$", "M"],
  });
  assert.strictEqual(policy.mapCommand(fightSummary), "CHOOSE 2");
});

test("map command waits when choices are not populated yet", () => {
  const summary = baseSummary({
    screen_type: "MAP",
    available_commands: ["choose", "state"],
    choices: [],
  });
  assert.strictEqual(policy.mapCommand(summary), "state");
});

test("map scoring ranks known room symbols deterministically", () => {
  assert(policy.mapChoiceScore("E") > policy.mapChoiceScore("M"));
  assert(policy.mapChoiceScore("M") > policy.mapChoiceScore("T"));
  assert(policy.mapChoiceScore("T") > policy.mapChoiceScore("?"));
  assert(policy.mapChoiceScore("?") > policy.mapChoiceScore("$"));
  assert(policy.mapChoiceScore("$") > policy.mapChoiceScore("R"));
});

test("screen policies wait when choose is available but no choices are present", () => {
  for (const screen_type of ["GRID", "SHOP_ROOM", "REST"]) {
    const summary = baseSummary({ screen_type, available_commands: ["choose", "state"], choices: [] });
    assert.strictEqual(policy.nextCommand(summary), "state");
  }
});

test("repeated card reward choose can fall back to skip", () => {
  const summary = baseSummary({
    screen_type: "CARD_REWARD",
    available_commands: ["choose", "skip", "state"],
    choices: ["clash", "sentinel", "rupture"],
  });
  assert.strictEqual(policy.fallbackCommand(summary, "CHOOSE 0"), "SKIP");
});

test("unavailable proceed is rejected when command list only allows choose", () => {
  const summary = baseSummary({
    screen_type: "GRID",
    available_commands: ["choose", "state"],
  });
  assert.strictEqual(policy.commandIsAvailable(summary, "PROCEED"), false);
  assert.strictEqual(policy.commandIsAvailable(summary, "CHOOSE 0"), true);
});

test("combat command attacks the lowest living monster", () => {
  const summary = baseSummary({
    available_commands: ["play", "end", "state"],
    combat: {
      hand: [
        { index: 1, name: "Defend", playable: true, type: "SKILL", has_target: false },
        { index: 2, name: "Strike", playable: true, type: "ATTACK", has_target: true },
      ],
      monsters: [
        { index: 0, hp: 0, gone: true, intent: "ATTACK" },
        { index: 1, hp: 12, gone: false, intent: "ATTACK" },
        { index: 2, hp: 7, gone: false, intent: "ATTACK" },
      ],
    },
  });
  assert.strictEqual(policy.combatCommand(summary), "PLAY 2 2");
});

test("combat command blocks before striking when low hp faces heavy damage", () => {
  const summary = baseSummary({
    current_hp: 18,
    available_commands: ["play", "end", "state"],
    combat: {
      hand: [
        { index: 1, name: "Defend", playable: true, type: "SKILL", has_target: false },
        { index: 2, name: "Strike", playable: true, type: "ATTACK", has_target: true },
      ],
      monsters: [{ index: 0, hp: 20, gone: false, intent: "ATTACK 12" }],
    },
  });
  assert(policy.cardScore(summary.combat.hand[0], 12, 18) > policy.cardScore(summary.combat.hand[1], 12, 18));
  assert.strictEqual(policy.combatCommand(summary), "PLAY 1");
});

test("state signature changes when choices change", () => {
  const first = policy.stateSignature(baseSummary({ choices: ["potion"] }));
  const second = policy.stateSignature(baseSummary({ choices: ["card"] }));
  assert.notStrictEqual(first, second);
});

test("stale collector exits when session files stop changing", () => {
  const reason = policy.staleSessionReasonFrom({
    summary: { ready_for_command: true },
    summaryAgeMs: 121000,
    status: { status: "sent" },
    statusAgeMs: 122000,
    maxIdleThresholdMs: 120000,
  });
  assert.match(reason, /session idle/);
});

test("stale collector exits immediately when bridge reports exited", () => {
  const reason = policy.staleSessionReasonFrom({
    summary: { ready_for_command: true },
    summaryAgeMs: 10,
    status: { status: "exited", reason: "stdin_closed" },
    statusAgeMs: 10,
    maxIdleThresholdMs: 120000,
  });
  assert.match(reason, /bridge exited: stdin_closed/);
});
