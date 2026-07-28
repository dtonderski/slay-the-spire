#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const path = require("path");
const {
  acquireDirectoryLock,
  addCollectionMetadata,
  chooseRandomAction,
  currentRunRecords,
  enumerateGameplayActions,
  expectedFailureBoundary,
  fingerprint,
  immutableTracePath,
  isPromotableFailure,
  isSoleEventLeaveScreen,
  loadBossUnlocks,
  localBridgeTracePath,
  needsGameplaySettlePoll,
  needsMapChoiceSettle,
  normalizeSettledGameplayRecords,
  parseParityOutput,
  parseBossUnlocks,
  parseSeenBossesPreferences,
  seededRandom,
  semanticStateKey,
  shouldVerifyTrace,
  stateAdvanced,
  verificationCheckpointKey,
  verifierInvocationFailed,
  writeTrace,
} = require("./random_fidelity_collector");

const traceWriteRoot = fs.mkdtempSync(path.join("/tmp", "sts-trace-write-"));
try {
  const tracePath = path.join(traceWriteRoot, "trace.jsonl");
  writeTrace(tracePath, [{ type: "metadata" }, { type: "action", command: "END" }], {
    exclusive: true,
  });
  assert.strictEqual(
    fs.readFileSync(tracePath, "utf8"),
    '{"type":"metadata"}\n{"type":"action","command":"END"}\n',
  );
  assert.throws(
    () => writeTrace(tracePath, [{ type: "metadata" }], { exclusive: true }),
    (error) => error.code === "EEXIST",
  );
} finally {
  fs.rmSync(traceWriteRoot, { recursive: true, force: true });
}

const lockTestRoot = fs.mkdtempSync(path.join("/tmp", "sts-manifest-lock-"));
try {
  const staleLock = path.join(lockTestRoot, "stale");
  fs.mkdirSync(staleLock);
  acquireDirectoryLock(staleLock, 100, 0);
  fs.rmdirSync(staleLock);

  const freshLock = path.join(lockTestRoot, "fresh");
  fs.mkdirSync(freshLock);
  assert.throws(
    () => acquireDirectoryLock(freshLock, 25, 60_000),
    /timed out locking/,
  );
  fs.rmdirSync(freshLock);
} finally {
  fs.rmSync(lockTestRoot, { recursive: true, force: true });
}

assert.strictEqual(
  needsMapChoiceSettle({ screen_type: "MAP", available_commands: ["return", "state"], choices: [] }),
  true,
);
assert.strictEqual(
  isSoleEventLeaveScreen({ room_type: "NeowRoom", screen_type: "EVENT", choices: ["leave"] }),
  true,
);
assert.strictEqual(
  isSoleEventLeaveScreen({ room_type: "EventRoom", screen_type: "EVENT", choices: ["leave"] }),
  true,
);
assert.strictEqual(
  needsMapChoiceSettle({ screen_type: "MAP", available_commands: ["choose", "return"], choices: ["x=0"] }),
  false,
);

const beforeSummary = {
  step: 10,
  state_id: "before",
  state_seq: 1,
  playtime_seconds: 0.1,
  choices: ["talk"],
  floor: 0,
};
const beforeKey = semanticStateKey({ summary: beforeSummary });
assert.strictEqual(
  stateAdvanced(beforeKey, {
    summary: { ...beforeSummary, step: 11, state_id: "after", state_seq: 2, playtime_seconds: 0.2 },
  }),
  false,
);
assert.strictEqual(
  stateAdvanced(beforeKey, { summary: { ...beforeSummary, choices: ["upgrade a card"] } }),
  true,
);
assert.strictEqual(stateAdvanced(beforeKey, {}), false);
const gridBefore = {
  state: { message: { game_state: { screen_type: "GRID", screen_state: { selected_cards: [] } } } },
};
const gridAfter = {
  state: {
    message: {
      game_state: {
        screen_type: "GRID",
        screen_state: { selected_cards: [{ id: "Defend_R" }] },
      },
    },
  },
};
assert.strictEqual(stateAdvanced(semanticStateKey(gridBefore), gridAfter), true);
const readyGridAfter = { ...gridAfter, summary: { ready_for_command: true } };
assert.strictEqual(needsGameplaySettlePoll(semanticStateKey(gridBefore), readyGridAfter), false);
assert.strictEqual(
  needsGameplaySettlePoll(semanticStateKey(gridBefore), { ...gridAfter, summary: { ready_for_command: false } }),
  true,
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose"],
    choices: ["stolen_gold", "gold", "card", "potion"],
  }),
  ["CHOOSE 0", "CHOOSE 1"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose"],
    room_type: "NeowRoom",
    choices: ["transform a card", "obtain 3 random potions", "obtain 100 gold"],
  }),
  ["CHOOSE 0", "CHOOSE 2"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({ available_commands: ["return"], screen_type: "MAP" }),
  ["RETURN"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose", "return"],
    screen_type: "MAP",
    choices: ["x=4", "x=5"],
  }),
  ["CHOOSE 0", "CHOOSE 1"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose"],
    choices: ["gather gold", "leave it"],
  }),
  ["CHOOSE 1"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose"],
    choices: ["touch", "trade", "leave"],
  }),
  ["CHOOSE 0", "CHOOSE 2"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose", "proceed"],
    screen_type: "COMBAT_REWARD",
    choices: ["gold", "card"],
  }),
  ["CHOOSE 0", "CHOOSE 1"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose", "proceed"],
    screen_type: "COMBAT_REWARD",
    open_potion_slots: 0,
    choices: ["potion"],
  }),
  ["PROCEED"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose", "skip"],
    room_type: "NeowRoom",
    screen_type: "CARD_REWARD",
    choices: ["forethought", "swift strike"],
  }),
  ["CHOOSE 0", "CHOOSE 1"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose"],
    screen_type: "COMBAT_REWARD",
    open_potion_slots: 1,
    choices: ["gold", "potion", "card"],
  }),
  ["CHOOSE 0", "CHOOSE 2"],
);

const checkpoint = verificationCheckpointKey({
  in_game: true,
  act: 2,
  floor: 23,
  room_type: "MONSTER",
  room_phase: "COMBAT",
  screen_type: "HAND_SELECT",
});
assert.strictEqual(
  checkpoint,
  verificationCheckpointKey({
    in_game: true,
    act: 2,
    floor: 23,
    room_type: "MONSTER",
    room_phase: "COMBAT",
    screen_type: "NONE",
  }),
);
assert.strictEqual(
  checkpoint,
  verificationCheckpointKey({
    in_game: true,
    act: 2,
    floor: 23,
    room_type: "MONSTER",
    room_phase: "COMPLETE",
  }),
);
assert.strictEqual(shouldVerifyTrace({
  actionCount: 0,
  lastVerifiedActionCount: null,
  checkpointKey: checkpoint,
  lastVerifiedCheckpointKey: null,
  interval: 50,
  terminal: false,
}), true);
assert.strictEqual(shouldVerifyTrace({
  actionCount: 49,
  lastVerifiedActionCount: 0,
  checkpointKey: checkpoint,
  lastVerifiedCheckpointKey: checkpoint,
  interval: 50,
  terminal: false,
}), false);
assert.strictEqual(shouldVerifyTrace({
  actionCount: 50,
  lastVerifiedActionCount: 0,
  checkpointKey: checkpoint,
  lastVerifiedCheckpointKey: checkpoint,
  interval: 50,
  terminal: false,
}), true);
assert.strictEqual(shouldVerifyTrace({
  actionCount: 1,
  lastVerifiedActionCount: 0,
  checkpointKey: verificationCheckpointKey({ in_game: true, floor: 24 }),
  lastVerifiedCheckpointKey: checkpoint,
  interval: 50,
  terminal: false,
}), true);
assert.strictEqual(shouldVerifyTrace({
  actionCount: 1,
  lastVerifiedActionCount: 0,
  checkpointKey: checkpoint,
  lastVerifiedCheckpointKey: checkpoint,
  interval: 50,
  terminal: true,
}), true);

const bossUnlocks = parseBossUnlocks(JSON.stringify({
  guardian_seen: false,
  hexaghost_seen: true,
  slime_boss_seen: true,
  champ_seen: true,
  automaton_seen: true,
  collector_seen: false,
  awakened_one_seen: true,
  donu_deca_seen: false,
  time_eater_seen: true,
}));
assert.strictEqual(bossUnlocks.guardian_seen, false);
assert.throws(() => parseBossUnlocks("{}"), /guardian_seen/);
assert.throws(() => parseBossUnlocks(), /required/);
assert.throws(() => loadBossUnlocks({}), /STS_SEEN_BOSSES_PATH/);
assert.deepStrictEqual(
  parseSeenBossesPreferences(JSON.stringify({
    GUARDIAN: "1",
    GHOST: "1",
    SLIME: "1",
    CHAMP: "1",
    AUTOMATON: "1",
    COLLECTOR: "1",
    CROW: "1",
    DONUT: "1",
  })),
  {
    guardian_seen: true,
    hexaghost_seen: true,
    slime_boss_seen: true,
    champ_seen: true,
    automaton_seen: true,
    collector_seen: true,
    awakened_one_seen: true,
    donu_deca_seen: true,
    time_eater_seen: false,
  },
);
const enriched = addCollectionMetadata(
  [{ type: "metadata", source: "bridge" }, { type: "action", step: 1 }],
  bossUnlocks,
  7,
  "SEED7",
  10000,
  "test-source-v1",
);
assert.strictEqual(enriched.length, 2);
assert.deepStrictEqual(enriched[0].boss_unlocks, bossUnlocks);
assert.strictEqual(enriched[0].schema, 1);
assert.strictEqual(enriched[0].source_version, "test-source-v1");
assert.deepStrictEqual(enriched[0].collection, {
  policy_seed: 7,
  game_seed: "SEED7",
  starting_hp: 10000,
});
assert.strictEqual(
  immutableTracePath(
    "/campaign",
    "SEED/7",
    7,
    new Date("2026-07-25T01:02:03.456Z"),
    123,
  ),
  path.join(
    "/campaign",
    "traces",
    "SEED_7-p7-2026-07-25T01-02-03-456Z-123.jsonl",
  ),
);
const normalized = normalizeSettledGameplayRecords([
  { type: "action", step: 10, command: "CHOOSE 0" },
  { type: "state", step: 10, message: { game_state: { choice_list: ["talk"] } } },
  {
    type: "action",
    step: 11,
    command: "CHOOSE 0",
    command_meta: {
      metadata: {
        operator_control: "settle_gameplay",
        reason: "confirm_event_leave",
      },
    },
  },
  { type: "state", step: 11, message: { game_state: { choice_list: ["talk"] } } },
  {
    type: "action",
    step: 12,
    command: "STATE",
    command_meta: { metadata: { operator_control: "settle_gameplay" } },
  },
  { type: "state", step: 12, message: { game_state: { choice_list: ["upgrade"] } } },
]);
assert.deepStrictEqual(normalized, [
  { type: "action", step: 10, command: "CHOOSE 0" },
  { type: "state", step: 10, message: { game_state: { choice_list: ["talk"] } } },
  {
    type: "action",
    step: 11,
    command: "CHOOSE 0",
    command_meta: {
      metadata: {
        operator_control: "settle_gameplay",
        reason: "confirm_event_leave",
      },
    },
  },
  { type: "state", step: 11, message: { game_state: { choice_list: ["upgrade"] } } },
]);
const foldedSelectedLeave = normalizeSettledGameplayRecords([
  { type: "action", step: 20, command: "CHOOSE 1" },
  { type: "state", step: 20, message: { game_state: { choice_list: ["leave"] } } },
  {
    type: "action",
    step: 21,
    command: "CHOOSE 0",
    command_meta: {
      metadata: {
        operator_control: "settle_gameplay",
        reason: "confirm_selected_event_leave",
      },
    },
  },
  { type: "state", step: 21, message: { game_state: { screen_type: "MAP" } } },
]);
assert.deepStrictEqual(foldedSelectedLeave, [
  { type: "action", step: 20, command: "CHOOSE 1" },
  { type: "state", step: 20, message: { game_state: { screen_type: "MAP" } } },
]);

const summary = {
  available_commands: ["play", "end", "potion", "state", "abandon"],
  combat: {
    discard_pile_count: 2,
    hand: [
      { index: 1, playable: true, has_target: true },
      { index: 2, playable: false, has_target: false },
      { index: 3, playable: true, has_target: false },
      { index: 4, id: "Headbutt", playable: true, has_target: true },
      { index: 5, id: "Armaments", playable: true, has_target: false },
      { index: 6, id: "Sword Boomerang", playable: true, has_target: false },
      { index: 7, id: "Dual Wield", playable: true, has_target: false },
      { index: 8, id: "Exhume", playable: true, has_target: false },
      { index: 9, id: "True Grit", playable: true, has_target: false },
    ],
    monsters: [
      { index: 0, hp: 10, gone: false, half_dead: false },
      { index: 1, hp: 0, gone: true, half_dead: false },
    ],
  },
  potions: [
    { index: 0, can_use: true, requires_target: true },
    { index: 1, id: "LiquidMemories", can_use: true, requires_target: false },
    { index: 2, id: "PowerPotion", can_use: true, requires_target: false },
    { index: 3, id: "GamblersBrew", can_use: true, requires_target: false },
  ],
};

assert.deepStrictEqual(enumerateGameplayActions(summary), [
  "PLAY 1 0",
  "PLAY 3",
  "POTION USE 0 0",
  "END",
]);
assert.ok(!enumerateGameplayActions(summary).some((action) => /STATE|ABANDON/.test(action)));
assert.ok(!enumerateGameplayActions(summary).some((action) => action.startsWith("PLAY 4")));
assert.ok(!enumerateGameplayActions(summary).includes("PLAY 5"));
assert.ok(!enumerateGameplayActions(summary).includes("PLAY 6"));
assert.ok(!enumerateGameplayActions(summary).includes("PLAY 7"));
assert.ok(!enumerateGameplayActions(summary).includes("PLAY 8"));
assert.ok(!enumerateGameplayActions(summary).includes("PLAY 9"));
assert.ok(!enumerateGameplayActions(summary).includes("POTION USE 1"));
assert.ok(!enumerateGameplayActions(summary).includes("POTION USE 2"));
assert.ok(!enumerateGameplayActions(summary).includes("POTION USE 3"));
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["key"],
    screen_type: "NONE",
    screen_name: "MASTER_DECK_VIEW",
    room_type: "TreasureRoomBoss",
  }),
  ["KEY CANCEL"],
);
assert.deepStrictEqual(
  enumerateGameplayActions({
    available_commands: ["choose", "leave"],
    screen_type: "SHOP_SCREEN",
    choices: ["purge", "weak potion", "strength potion", "armaments"],
  }),
  ["CHOOSE 0", "CHOOSE 3", "LEAVE"],
);
assert.ok(
  enumerateGameplayActions({
    ...summary,
    combat: { ...summary.combat, discard_pile_count: 1 },
  }).includes("PLAY 4 0"),
);

const first = seededRandom(1234);
const second = seededRandom(1234);
assert.deepStrictEqual(
  Array.from({ length: 20 }, () => chooseRandomAction(summary, first)),
  Array.from({ length: 20 }, () => chooseRandomAction(summary, second)),
);

const parsed = parseParityOutput(`outcome=failed\nunexpected_diffs=1\nduplicate_dispositions=0\nseed_start.first_boundary.path=$.actions[step=99].command\nseed_start.first_boundary.category=unsupported_combat_path\nunexpected_diff step=12 command="END" label="combat end turn"`);
assert.strictEqual(parsed.unexpectedDiffs, 1);
assert.strictEqual(parsed.firstDiff.label, "combat end turn");
assert.deepStrictEqual(parsed.diffLines, []);
assert.deepStrictEqual(expectedFailureBoundary(parsed), {
  path: "$.actions[step=12].command",
  category: "unexpected_sim_real_diff",
});
assert.deepStrictEqual(
  expectedFailureBoundary({
    ...parsed,
    boundaryPath: "$.actions[step=12].command",
    boundaryCategory: "unsupported_combat_path",
  }),
  { path: "$.actions[step=12].command", category: "unsupported_combat_path" },
);
assert.strictEqual(isPromotableFailure(parsed), true);
assert.strictEqual(isPromotableFailure({ ...parsed, duplicateDispositions: 1 }), false);
assert.strictEqual(isPromotableFailure({
  ...parsed,
  boundaryPath: "$.actions[step=12].command",
  boundaryCategory: "unsupported_combat_path",
}), false);
assert.strictEqual(isPromotableFailure({
  ...parsed,
  boundaryPath: "$.actions[step=99].command",
  boundaryCategory: "unsupported_neow_boss_swap",
}), false);
assert.strictEqual(fingerprint(parsed), fingerprint({ ...parsed, boundaryPath: "$.actions[step=99]" }));
assert.strictEqual(verifierInvocationFailed({ status: 2, error: { code: "EPERM" } }), false);
assert.strictEqual(verifierInvocationFailed({ status: 0, error: { code: "EPERM" } }), false);
assert.strictEqual(verifierInvocationFailed({ status: null, error: { code: "EPERM" } }), true);

async function testOffsetRunRead() {
  const directory = fs.mkdtempSync(path.join(require("os").tmpdir(), "random-fidelity-offset-"));
  const tracePath = path.join(directory, "bridge.jsonl");
  const oldRun = `${JSON.stringify({ type: "action", step: 1, command: "START OLD" })}\n`;
  fs.writeFileSync(tracePath, oldRun);
  const startOffset = fs.statSync(tracePath).size;
  fs.appendFileSync(
    tracePath,
    [
      { type: "action", step: 900, command: "START_VERIFY IRONCLAD 0 NEW 10000" },
      { type: "state", step: 900, message: { game_state: { floor: 0 } } },
      { type: "action", step: 901, command: "CHOOSE 0" },
      { type: "state", step: 901, message: { game_state: { floor: 1 } } },
    ].map((record) => JSON.stringify(record)).join("\n") + "\n",
  );
  const records = await currentRunRecords(tracePath, startOffset);
  assert.strictEqual(records.some((record) => record.command === "START OLD"), false);
  assert.strictEqual(records.find((record) => record.type === "action").step, 1);
  assert.strictEqual(records.at(-1).step, 2);
  fs.rmSync(directory, { recursive: true, force: true });
}

async function testWindowsBridgePathTranslation() {
  if (process.platform === "win32") return;
  assert.strictEqual(
    localBridgeTracePath(
      "D:\\dev\\slay-the-spire\\tools\\communication\\session\\raw_bridge_current.jsonl",
    ),
    "/mnt/d/dev/slay-the-spire/tools/communication/session/raw_bridge_current.jsonl",
  );
}

Promise.all([testOffsetRunRead(), testWindowsBridgePathTranslation()])
  .then(() => console.log("random_fidelity_collector tests passed"))
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
