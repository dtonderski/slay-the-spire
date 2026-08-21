#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const {
  addCollectionMetadata,
  chooseRandomAction,
  communicationBoundary,
  createCombatStallTracker,
  currentRunRecords,
  enumerateGameplayActions,
  immutableTracePath,
  isSoleEventLeaveScreen,
  loadBossUnlocks,
  localBridgeTracePath,
  menuStartReady,
  needsMapChoiceSettle,
  normalizeSettledGameplayRecords,
  observeCombatStall,
  parseBossUnlocks,
  parseSeenBossesPreferences,
  playerCombatStrength,
  seededRandom,
  totalLivingEnemyHp,
  validateProfileSnapshot,
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
assert.strictEqual(
  menuStartReady({
    in_game: false,
    ready_for_command: false,
    available_commands: ["start", "start_verify", "state", "profile"],
  }),
  false,
);
assert.strictEqual(
  menuStartReady({
    in_game: false,
    ready_for_command: true,
    available_commands: ["start", "start_verify", "state", "profile"],
  }),
  true,
);
assert.strictEqual(
  menuStartReady({
    in_game: true,
    ready_for_command: true,
    available_commands: ["start_verify", "state"],
  }),
  false,
);

const boundaryMessage = {
  boundary_schema: 6,
  boundary_kind: "interaction_ready",
  ready_for_command: true,
  game_update_seq: 100,
  dungeon_update_seq: 90,
  command_execution_seq: 12,
  effects_size: 0,
  top_level_effects_size: 0,
  queued_top_level_effects_size: 0,
  current_action: "DiscoveryAction",
  current_action_instance: 7,
  current_action_update_count: 3,
  actions_queued: 1,
  card_queue_size: 0,
  pre_turn_actions_size: 0,
  end_turn_queued: false,
};
const interactionBoundary = { state: { message: boundaryMessage } };
assert.strictEqual(
  communicationBoundary(interactionBoundary).kind,
  "interaction_ready",
);
assert.throws(
  () => communicationBoundary({ message: { error: "Index 5 out of bounds" } }),
  /CommunicationMod rejected command: Index 5 out of bounds/,
);
assert.throws(
  () => communicationBoundary({ message: { ...boundaryMessage, boundary_schema: 1 } }),
  /boundary_schema=6 is required/,
);
assert.throws(
  () => communicationBoundary({ message: { ...boundaryMessage, boundary_kind: "quiescent" } }),
  /quiescent.*queued work/,
);
assert.throws(
  () => communicationBoundary({ message: { ...boundaryMessage, ready_for_command: undefined } }),
  /interaction_ready.*not ready for input/,
);
for (const invalid of ["1", null, false, 1.5]) {
  assert.throws(
    () => communicationBoundary({ message: { ...boundaryMessage, boundary_schema: invalid } }),
    /boundary_schema=6 is required/,
  );
}
assert.throws(
  () => communicationBoundary({ message: { ...boundaryMessage, end_turn_queued: undefined } }),
  /requires boolean end_turn_queued/,
);
assert.strictEqual(
  communicationBoundary({ message: { ...boundaryMessage, end_turn_queued: true } }).kind,
  "interaction_ready",
);
assert.throws(
  () => communicationBoundary({
    message: {
      ...boundaryMessage,
      boundary_kind: "quiescent",
      current_action: null,
      actions_queued: 0,
      end_turn_queued: true,
    },
  }),
  /cannot have an end turn queued/,
);
for (const field of [
  "effects_size",
  "top_level_effects_size",
  "queued_top_level_effects_size",
]) {
  assert.throws(
    () => communicationBoundary({ message: { ...boundaryMessage, [field]: 1 } }),
    new RegExp(`${field}=0`),
  );
}
for (const invalid of ["100", null, false, 1.5]) {
  assert.throws(
    () => communicationBoundary({ message: { ...boundaryMessage, game_update_seq: invalid } }),
    /game_update_seq must be a non-negative integer/,
  );
}
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
// STS persists Time Eater as WIZARD in STSSeenBosses.
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
    WIZARD: "1",
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
    time_eater_seen: true,
  },
);
{
  const prefsPath = path.join(os.tmpdir(), `sts-seen-bosses-${process.pid}.json`);
  fs.writeFileSync(
    prefsPath,
    JSON.stringify({
      GUARDIAN: "1",
      GHOST: "1",
      SLIME: "1",
      CHAMP: "1",
      AUTOMATON: "1",
      COLLECTOR: "1",
      CROW: "1",
      DONUT: "1",
      WIZARD: "1",
    }),
  );
  try {
    assert.strictEqual(
      loadBossUnlocks({
        STS_SEEN_BOSSES_PATH: prefsPath,
        STS_BOSS_UNLOCKS_JSON: JSON.stringify({
          guardian_seen: true,
          hexaghost_seen: true,
          slime_boss_seen: true,
          champ_seen: true,
          automaton_seen: true,
          collector_seen: true,
          awakened_one_seen: true,
          donu_deca_seen: true,
          time_eater_seen: false,
        }),
      }).time_eater_seen,
      true,
      "live prefs path must win over a stale unlocks JSON",
    );
  } finally {
    fs.rmSync(prefsPath, { force: true });
  }
}
const enriched = addCollectionMetadata(
  [{ type: "metadata", source: "bridge" }, { type: "action", step: 1 }],
  bossUnlocks,
  7,
  "SEED7",
  10000,
  { note_card: "Strike_R", note_upgrades: 1, final_act_available: true },
  "test-source-v1",
  6,
);
assert.strictEqual(enriched.length, 2);
assert.deepStrictEqual(enriched[0].boss_unlocks, bossUnlocks);
assert.strictEqual(enriched[0].schema, 1);
assert.strictEqual(enriched[0].boundary_schema, 6);
assert.strictEqual(enriched[0].source_version, "test-source-v1");
assert.throws(
  () => addCollectionMetadata(
    [{ type: "state", step: 1, message: { boundary_schema: 1 } }],
    bossUnlocks,
    7,
    "SEED7",
    10000,
    { note_card: "Strike_R", note_upgrades: 1, final_act_available: true },
    "test-source-v1",
    6,
  ),
  /trace boundary_schema changed from 6 to 1/,
);
assert.deepStrictEqual(enriched[0].run_config.profile, {
  note_card: "Strike_R",
  note_upgrades: 1,
  final_act_available: true,
});
assert.deepStrictEqual(
  validateProfileSnapshot({
    note_card: "Strike_R",
    note_upgrades: 1,
    final_act_available: false,
  }),
  { note_card: "Strike_R", note_upgrades: 1, final_act_available: false },
);
assert.deepStrictEqual(
  validateProfileSnapshot({ note_upgrades: 0, final_act_available: true }),
  { note_upgrades: 0, final_act_available: true },
);
assert.throws(
  () => validateProfileSnapshot({ note_upgrades: 1, final_act_available: true }),
  /note_card/,
);
assert.throws(
  () => validateProfileSnapshot({ note_card: "Strike_R", note_upgrades: 1 }),
  /final_act_available/,
);
assert.throws(
  () => validateProfileSnapshot({
    note_card: "Strike_R",
    note_upgrades: 1,
    final_act_available: "false",
  }),
  /final_act_available/,
);
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
assert.throws(
  () => normalizeSettledGameplayRecords([
    { type: "action", step: 20, command: "CHOOSE 1" },
    { type: "state", step: 20, message: { game_state: { choice_list: ["leave"] } } },
  ]),
  /boundary_schema=6 is required/,
);

const fencedAction = (step, command, sourceCommandExecutionSeq = 11) => ({
  type: "action",
  step,
  command,
  command_meta: { source_command_execution_seq: sourceCommandExecutionSeq },
});
const quiescentBoundaryMessage = {
  ...boundaryMessage,
  boundary_kind: "quiescent",
  current_action: null,
  current_action_instance: null,
  current_action_update_count: null,
  actions_queued: 0,
};
const schemaTwoRecords = [
  fencedAction(30, "END"),
  { type: "metadata", event: "command_sent", step: 30 },
  { type: "state", step: 30, message: quiescentBoundaryMessage },
];
assert.deepStrictEqual(normalizeSettledGameplayRecords(schemaTwoRecords), schemaTwoRecords);
assert.deepStrictEqual(
  normalizeSettledGameplayRecords(normalizeSettledGameplayRecords(schemaTwoRecords)),
  schemaTwoRecords,
);
const externalRng = { type: "external_rng", step: 30, draws: [{ stream: "misc", value: 2 }] };
assert.deepStrictEqual(
  normalizeSettledGameplayRecords([schemaTwoRecords[0], externalRng, schemaTwoRecords[2]]),
  [schemaTwoRecords[0], externalRng, schemaTwoRecords[2]],
);
const rejectedRecords = [
  { type: "action", step: 33, command: "CHOOSE 99" },
  { type: "error", step: 33, message: { error: "invalid choice" } },
];
assert.deepStrictEqual(normalizeSettledGameplayRecords(rejectedRecords), rejectedRecords);
assert.throws(
  () => normalizeSettledGameplayRecords([
    fencedAction(34, "END"),
    { type: "state", step: 35, message: quiescentBoundaryMessage },
  ]),
  /state step 35 does not match action step 34/,
);
assert.throws(
  () => normalizeSettledGameplayRecords([
    fencedAction(34, "END"),
    { type: "external_rng", step: 35, draws: [] },
    { type: "state", step: 34, message: quiescentBoundaryMessage },
  ]),
  /external_rng step 35 does not match action step 34/,
);
assert.throws(
  () => normalizeSettledGameplayRecords([
    { type: "error", step: 34, message: { error: "orphan" } },
  ]),
  /orphan error record/,
);
assert.throws(
  () => normalizeSettledGameplayRecords([fencedAction(30, "END")]),
  /produced missing, not a completing boundary/,
);
assert.throws(
  () => normalizeSettledGameplayRecords([
    ...schemaTwoRecords,
    fencedAction(31, "END"),
    { type: "state", step: 31, message: { game_state: {} } },
  ]),
  /boundary_schema=6 is required/,
);
const schemaTwoStateRecords = [
  { type: "action", step: 31, command: "STATE" },
  { type: "state", step: 31, message: quiescentBoundaryMessage },
  {
    type: "state",
    step: 31,
    message: { ...quiescentBoundaryMessage, boundary_kind: "poll" },
  },
];
assert.deepStrictEqual(
  normalizeSettledGameplayRecords(schemaTwoStateRecords),
  [schemaTwoStateRecords[0], schemaTwoStateRecords[2]],
);
const schemaTwoOvertakenGameplayRecords = [
  fencedAction(32, "CHOOSE 0"),
  {
    type: "state",
    step: 32,
    message: { ...quiescentBoundaryMessage, boundary_kind: "unknown", ready_for_command: false },
  },
  {
    type: "state",
    step: 32,
    message: { ...quiescentBoundaryMessage, boundary_kind: "poll" },
  },
  { type: "state", step: 32, message: quiescentBoundaryMessage },
];
assert.deepStrictEqual(
  normalizeSettledGameplayRecords(schemaTwoOvertakenGameplayRecords),
  [schemaTwoOvertakenGameplayRecords[0], schemaTwoOvertakenGameplayRecords[3]],
);
const commandFenceOvertakeRecords = [
  fencedAction(33, "CHOOSE 3"),
  {
    type: "state",
    step: 33,
    message: { ...quiescentBoundaryMessage, command_execution_seq: 11 },
  },
  {
    type: "state",
    step: 33,
    message: { ...quiescentBoundaryMessage, command_execution_seq: 12 },
  },
];
assert.deepStrictEqual(
  normalizeSettledGameplayRecords(commandFenceOvertakeRecords),
  [commandFenceOvertakeRecords[0], commandFenceOvertakeRecords[2]],
);
assert.throws(
  () =>
    normalizeSettledGameplayRecords([
      fencedAction(31, "END"),
      {
        type: "state",
        step: 31,
        message: { ...quiescentBoundaryMessage, boundary_kind: "poll" },
      },
    ]),
  /gameplay action at step 31 produced poll, not a completing boundary/,
);

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
    choices: ["purge", "weak potion", "gambler's brew", "armaments"],
    shop_potions: [{ id: "GamblersBrew", name: "Gambler's Brew" }],
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

assert.strictEqual(playerCombatStrength({ combat: { player_strength: -12 } }), -12);
assert.strictEqual(playerCombatStrength({ combat: {} }), 0);
assert.strictEqual(
  totalLivingEnemyHp({
    combat: {
      monsters: [
        { hp: 32, gone: false, half_dead: false },
        { hp: 10, gone: true, half_dead: false },
        { hp: 0, gone: false, half_dead: false },
      ],
    },
  }),
  32,
);

{
  const tracker = createCombatStallTracker();
  const base = {
    in_game: true,
    combat: {
      turn: 10,
      player_strength: -11,
      monsters: [{ hp: 40, gone: false, half_dead: false }],
    },
  };
  assert.strictEqual(observeCombatStall(tracker, base).shouldAbandon, false);
  assert.strictEqual(
    observeCombatStall(tracker, {
      ...base,
      combat: { ...base.combat, turn: 11 },
    }).shouldAbandon,
    false,
  );
  assert.strictEqual(
    observeCombatStall(tracker, {
      ...base,
      combat: { ...base.combat, turn: 12 },
    }).shouldAbandon,
    false,
  );
  const third = observeCombatStall(tracker, {
    ...base,
    combat: { ...base.combat, turn: 13 },
  });
  assert.strictEqual(third.shouldAbandon, true);
  assert.match(third.reason, /player strength -11 < -10/);
  assert.match(third.reason, /unchanged for 3 turns/);

  const damageBreaksStall = createCombatStallTracker();
  observeCombatStall(damageBreaksStall, base);
  observeCombatStall(damageBreaksStall, {
    ...base,
    combat: { ...base.combat, turn: 11 },
  });
  observeCombatStall(damageBreaksStall, {
    ...base,
    combat: {
      ...base.combat,
      turn: 12,
      monsters: [{ hp: 39, gone: false, half_dead: false }],
    },
  });
  assert.strictEqual(
    observeCombatStall(damageBreaksStall, {
      ...base,
      combat: {
        ...base.combat,
        turn: 13,
        monsters: [{ hp: 39, gone: false, half_dead: false }],
      },
    }).shouldAbandon,
    false,
  );

  const highStrength = createCombatStallTracker();
  for (let turn = 10; turn <= 20; turn += 1) {
    assert.strictEqual(
      observeCombatStall(highStrength, {
        in_game: true,
        combat: {
          turn,
          player_strength: -10,
          monsters: [{ hp: 40, gone: false, half_dead: false }],
        },
      }).shouldAbandon,
      false,
    );
  }
}


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
