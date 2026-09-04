#!/usr/bin/env node

const crypto = require("crypto");
const fs = require("fs");
const net = require("net");
const path = require("path");
const readline = require("readline");

const root = path.resolve(__dirname, "..", "..", "..");
const defaultSessionDir = path.join(__dirname, "session");

function seededRandom(seed) {
  let state = Number(seed) >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function availableSet(summary) {
  return new Set((summary?.available_commands || []).map((item) => String(item).toLowerCase()));
}

function menuStartReady(summary) {
  const available = availableSet(summary);
  return summary?.in_game === false
    && summary?.ready_for_command === true
    && (available.has("start") || available.has("start_verify"));
}

function livingMonsters(summary) {
  return (summary?.combat?.monsters || []).filter(
    (monster) => !monster.gone && !monster.half_dead && Number(monster.hp) > 0,
  );
}

/**
 * Enumerate every concrete gameplay command advertised by CommunicationMod.
 * Known hangs and simulator divergences remain eligible: they are evidence,
 * not policy exceptions. STATE/PROFILE/ABANDON and singleton timer settling
 * are collector controls rather than random gameplay choices.
 */
function enumerateGameplayActions(summary) {
  const available = availableSet(summary);
  const actions = [];
  const choices = Array.isArray(summary?.choices) ? summary.choices : [];

  if (available.has("choose")) {
    choices.forEach((_choice, index) => actions.push(`CHOOSE ${index}`));
  }
  if (available.has("play")) {
    const monsters = livingMonsters(summary);
    for (const card of summary?.combat?.hand || []) {
      if (!card.playable) continue;
      if (card.has_target) {
        monsters.forEach((monster) => actions.push(`PLAY ${card.index} ${monster.index}`));
      } else {
        actions.push(`PLAY ${card.index}`);
      }
    }
  }
  if (available.has("potion")) {
    const monsters = livingMonsters(summary);
    for (const potion of summary?.potions || []) {
      if (potion.can_discard) actions.push(`POTION DISCARD ${potion.index}`);
      if (!potion.can_use) continue;
      if (potion.requires_target) {
        monsters.forEach((monster) => actions.push(`POTION USE ${potion.index} ${monster.index}`));
      } else {
        actions.push(`POTION USE ${potion.index}`);
      }
    }
  }
  if (
    available.has("key") &&
    String(summary?.screen_name).toUpperCase() === "MASTER_DECK_VIEW"
  ) {
    actions.push("KEY CANCEL 250");
  }
  if (available.has("click") && String(summary?.screen_name).toUpperCase() === "FTUE") {
    actions.push("CLICK LEFT 1080 700 250");
  }
  for (const [verb, command] of [
    ["end", "END"],
    ["proceed", "PROCEED"],
    ["return", "RETURN"],
    ["confirm", "CONFIRM"],
    ["cancel", "CANCEL"],
    ["skip", "SKIP"],
    ["leave", "LEAVE"],
  ]) {
    if (available.has(verb)) actions.push(command);
  }
  if (
    actions.length === 0
    && available.has("wait")
    && String(summary?.screen_type).toUpperCase() === "EVENT"
    && (!Array.isArray(summary?.choices) || summary.choices.length === 0)
    && !["choose", "proceed", "leave", "confirm"].some((verb) => available.has(verb))
  ) {
    // Finished Match and Keep (and similar leftover events) can publish
    // EVENT with a null/empty choice list and no proceed/leave while the
    // leave dialog is still behind a wait timer. WAIT lets that timer
    // elapse; CommunicationMod also skips the timer after the last pick.
    actions.push("WAIT 240");
  }
  return [...new Set(actions)];
}

function sampleRandomAction(summary, random) {
  const actions = enumerateGameplayActions(summary);
  if (actions.length === 0) return null;
  const selectedIndex = Math.floor(random() * actions.length);
  return { command: actions[selectedIndex], selectedIndex, actions };
}

function chooseRandomAction(summary, random) {
  return sampleRandomAction(summary, random)?.command ?? null;
}

const GAMEPLAY_BOUNDARY_KINDS = new Set(["interaction_ready", "quiescent", "terminal"]);
const REQUIRED_BOUNDARY_SCHEMA = 7;
const SCHEMA7_STATE_RESPONSE_KINDS = new Set(["settled", "poll", "unsolicited"]);

function isSafeNonNegativeInteger(value) {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function communicationBoundary(protocolState, { allowUnsettled = false } = {}) {
  const message =
    protocolState?.state?.message ?? protocolState?.message ?? protocolState?.summary ?? protocolState;
  if (message?.error) {
    throw new Error(`CommunicationMod rejected command: ${message.error}`);
  }
  const schema = message?.boundary_schema;
  if (schema !== REQUIRED_BOUNDARY_SCHEMA) {
    throw new Error(
      `CommunicationMod boundary_schema=${REQUIRED_BOUNDARY_SCHEMA} is required, received ${schema ?? "missing"}`,
    );
  }
  const kind = String(message?.boundary_kind ?? "");
  const allowedKinds = [...GAMEPLAY_BOUNDARY_KINDS, "poll"];
  if (allowUnsettled) allowedKinds.push("unknown");
  if (!allowedKinds.includes(kind)) {
    throw new Error(`unknown CommunicationMod boundary_kind: ${kind || "missing"}`);
  }
  for (const field of [
    "game_update_seq",
    "dungeon_update_seq",
    "actions_queued",
    "card_queue_size",
    "pre_turn_actions_size",
  ]) {
    const value = message?.[field];
    if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
      throw new Error(`CommunicationMod ${field} must be a non-negative integer`);
    }
  }
  if (message?.current_action != null) {
    for (const field of ["current_action_instance", "current_action_update_count"]) {
      const value = message?.[field];
      if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
        throw new Error(`CommunicationMod ${field} must identify the current action`);
      }
    }
  }
  if (kind === "quiescent") {
    if (
      message?.current_action != null ||
      ["actions_queued", "card_queue_size", "pre_turn_actions_size"].some(
        (field) => message[field] !== 0,
      )
    ) {
      throw new Error("quiescent CommunicationMod boundary has active or queued work");
    }
  }
  if (
    kind === "interaction_ready" &&
    message?.current_action == null &&
    ["actions_queued", "card_queue_size", "pre_turn_actions_size"].every(
      (field) => message[field] === 0,
    )
  ) {
    throw new Error("interaction_ready CommunicationMod boundary has no active or queued work");
  }
  if (typeof message?.end_turn_queued !== "boolean") {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires boolean end_turn_queued`,
    );
  }
  if (!isSafeNonNegativeInteger(message?.command_execution_seq)) {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires a non-negative integer command_execution_seq`,
    );
  }
  if (!isSafeNonNegativeInteger(message?.command_settlement_seq)) {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires a non-negative integer command_settlement_seq`,
    );
  }
  if (!SCHEMA7_STATE_RESPONSE_KINDS.has(message?.command_response_kind)) {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires state command_response_kind settled, poll, or unsolicited`,
    );
  }
  if (message?.command_response_id != null && (typeof message.command_response_id !== "string" || !message.command_response_id)) {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires command_response_id to be a nonempty string or null`,
    );
  }
  if (typeof message?.transaction_pending !== "boolean") {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires boolean transaction_pending`,
    );
  }
  if (
    (message.command_response_kind === "settled" || message.command_response_kind === "poll")
    && (message.command_response_id == null || message.transaction_pending)
  ) {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} completed state requires an ID and transaction_pending=false`,
    );
  }
  if (message.command_response_kind === "unsolicited" && message.command_response_id != null) {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} unsolicited state must not name a response ID`,
    );
  }
  for (const field of [
    "effects_size",
    "top_level_effects_size",
    "queued_top_level_effects_size",
  ]) {
    if (!isSafeNonNegativeInteger(message?.[field]) || message[field] !== 0) {
      throw new Error(
        `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires ${field}=0`,
      );
    }
  }
  if (GAMEPLAY_BOUNDARY_KINDS.has(kind)) {
    if (message?.ready_for_command !== true) {
      throw new Error(`${kind} CommunicationMod boundary is not ready for input`);
    }
    const terminalDeathWithResidualEndTurn =
      kind === "terminal" &&
      message?.in_game === true &&
      String(message?.game_state?.screen_type ?? message?.screen_type ?? "").toUpperCase() ===
        "GAME_OVER";
    if (
      message.end_turn_queued &&
      kind !== "interaction_ready" &&
      !terminalDeathWithResidualEndTurn
    ) {
      throw new Error(`${kind} CommunicationMod boundary cannot have an end turn queued`);
    }
  }
  return { kind, message };
}

function assertSchema7Rejection(action, response) {
  const commandId = action.command_meta?.command_id;
  const sourceExecution = action.command_meta?.source_command_execution_seq;
  const sourceSettlement = action.command_meta?.source_command_settlement_seq;
  const message = response.message ?? {};
  const verb = String(action.command ?? "").trim().split(/\s+/)[0].toLowerCase();
  const observationCommand = verb === "state" || verb === "profile";
  if (typeof commandId !== "string" || !commandId) {
    throw new Error(`schema-7 rejected action at step ${action.step} requires command_id`);
  }
  if (!isSafeNonNegativeInteger(sourceExecution) || !isSafeNonNegativeInteger(sourceSettlement)) {
    throw new Error(
      `schema-7 rejected action at step ${action.step} requires source execution/settlement sequences`,
    );
  }
  if (
    message.boundary_schema !== 7
    || message.command_response_id !== commandId
    || message.command_response_kind !== "rejected"
    || message.transaction_pending !== false
  ) {
    throw new Error(
      `schema-7 rejected action at step ${action.step} completion identity mismatch`,
    );
  }
  if (
    !isSafeNonNegativeInteger(message.command_execution_seq)
    || !isSafeNonNegativeInteger(message.command_settlement_seq)
    || message.command_execution_seq !== (
      observationCommand ? sourceExecution : sourceExecution + 1
    )
    || message.command_settlement_seq !== sourceSettlement
  ) {
    throw new Error(
      `schema-7 rejected action at step ${action.step} has incorrect execution/settlement sequences`,
    );
  }
}

function needsMapChoiceSettle(summary) {
  return (
    String(summary?.screen_type).toUpperCase() === "MAP" &&
    availableSet(summary).has("return") &&
    (!Array.isArray(summary?.choices) || summary.choices.length === 0)
  );
}

function isSoleEventLeaveScreen(summary) {
  return (
    String(summary?.screen_type).toUpperCase() === "EVENT" &&
    Array.isArray(summary?.choices) &&
    summary.choices.length === 1 &&
    String(summary.choices[0]).toLowerCase() === "leave"
  );
}

function isCommandInFlightHang(error) {
  return /bridge command did not complete after acceptance:/i.test(String(error?.message || error));
}

function controlRequest(control, payload, timeoutMs = 15000) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: control.host || "127.0.0.1", port: control.port });
    let buffer = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("timed out waiting for bridge control response"));
    }, timeoutMs);
    socket.setEncoding("utf8");
    socket.on("connect", () => socket.write(`${JSON.stringify(payload)}\n`));
    socket.on("data", (chunk) => {
      buffer += chunk;
      const newline = buffer.indexOf("\n");
      if (newline < 0) return;
      clearTimeout(timer);
      socket.end();
      try {
        resolve(JSON.parse(buffer.slice(0, newline)));
      } catch (error) {
        reject(error);
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

function writeTrace(filePath, records, { exclusive = false } = {}) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const temporaryPath = `${filePath}.tmp-${process.pid}-${crypto.randomUUID()}`;
  const descriptor = fs.openSync(temporaryPath, "wx");
  try {
    for (const record of records) {
      fs.writeSync(descriptor, `${JSON.stringify(record)}\n`);
    }
    fs.fsyncSync(descriptor);
  } catch (error) {
    fs.closeSync(descriptor);
    fs.rmSync(temporaryPath, { force: true });
    throw error;
  }
  fs.closeSync(descriptor);
  try {
    if (exclusive) {
      fs.linkSync(temporaryPath, filePath);
      fs.unlinkSync(temporaryPath);
    } else {
      fs.renameSync(temporaryPath, filePath);
    }
  } catch (error) {
    fs.rmSync(temporaryPath, { force: true });
    throw error;
  }
}

function immutableTracePath(campaignDir, gameSeed, policySeed, startedAt, pid = process.pid) {
  const timestamp = startedAt.toISOString().replace(/[:.]/g, "-");
  const safeSeed = String(gameSeed).replace(/[^A-Za-z0-9_-]/g, "_");
  return path.join(
    campaignDir,
    "traces",
    `${safeSeed}-p${policySeed}-${timestamp}-${pid}.jsonl`,
  );
}

function localBridgeTracePath(advertisedPath, sessionDir = defaultSessionDir) {
  if (advertisedPath && fs.existsSync(advertisedPath)) return advertisedPath;
  const windowsPath = String(advertisedPath || "").match(/^([A-Za-z]):[\\/](.*)$/);
  if (windowsPath && process.platform !== "win32") {
    const translated = path.join(
      "/mnt",
      windowsPath[1].toLowerCase(),
      ...windowsPath[2].split(/[\\/]+/),
    );
    if (fs.existsSync(translated)) return translated;
  }
  const sessionTrace = path.join(sessionDir, path.basename(String(advertisedPath || "").replaceAll("\\", "/")));
  if (fs.existsSync(sessionTrace)) return sessionTrace;
  throw new Error(`bridge trace path is not readable locally: ${advertisedPath}`);
}

const bossUnlockKeys = [
  "guardian_seen",
  "hexaghost_seen",
  "slime_boss_seen",
  "champ_seen",
  "automaton_seen",
  "collector_seen",
  "awakened_one_seen",
  "donu_deca_seen",
  "time_eater_seen",
];

function parseBossUnlocks(value) {
  if (!value) {
    throw new Error(
      "STS_BOSS_UNLOCKS_JSON or STS_SEEN_BOSSES_PATH is required because boss selection depends on profile discovery state",
    );
  }
  let parsed;
  try {
    parsed = JSON.parse(value);
  } catch (error) {
    throw new Error(`invalid STS_BOSS_UNLOCKS_JSON: ${error.message}`);
  }
  for (const key of bossUnlockKeys) {
    if (typeof parsed[key] !== "boolean") {
      throw new Error(`STS_BOSS_UNLOCKS_JSON must contain boolean ${key}`);
    }
  }
  return Object.fromEntries(bossUnlockKeys.map((key) => [key, parsed[key]]));
}

function parseSeenBossesPreferences(value) {
  let parsed;
  try {
    parsed = JSON.parse(value);
  } catch (error) {
    throw new Error(`invalid STSSeenBosses preferences: ${error.message}`);
  }
  const seen = (...keys) => keys.some((key) => Number.parseInt(parsed[key] || "0", 10) > 0);
  return {
    guardian_seen: seen("GUARDIAN"),
    hexaghost_seen: seen("GHOST", "HEXAGHOST"),
    slime_boss_seen: seen("SLIME", "SLIME BOSS"),
    champ_seen: seen("CHAMP"),
    automaton_seen: seen("AUTOMATON"),
    collector_seen: seen("COLLECTOR"),
    awakened_one_seen: seen("CROW", "AWAKENED ONE"),
    donu_deca_seen: seen("DONUT", "DONU AND DECA"),
    // STS stores Time Eater under the prefs key WIZARD (not TIME).
    time_eater_seen: seen("WIZARD", "TIME", "TIME EATER", "TIME_EATER"),
  };
}

function loadBossUnlocks(environment = process.env) {
  // Prefer live STS prefs when available so a stale STS_BOSS_UNLOCKS_JSON cannot
  // override the profile the game is actually using.
  if (environment.STS_SEEN_BOSSES_PATH) {
    const preferencesPath = path.resolve(environment.STS_SEEN_BOSSES_PATH);
    return parseSeenBossesPreferences(fs.readFileSync(preferencesPath, "utf8"));
  }
  if (environment.STS_BOSS_UNLOCKS_JSON) {
    return parseBossUnlocks(environment.STS_BOSS_UNLOCKS_JSON);
  }
  return parseBossUnlocks();
}

function validateProfileSnapshot(profile) {
  const noteCardIsValid = profile?.note_card === undefined
    || (typeof profile.note_card === "string" && profile.note_card.trim() !== "");
  if (
    !noteCardIsValid ||
    !Number.isSafeInteger(profile?.note_upgrades) ||
    profile.note_upgrades < 0 ||
    profile.note_upgrades > 255 ||
    (profile.note_card === undefined && profile.note_upgrades !== 0) ||
    typeof profile.final_act_available !== "boolean"
  ) {
    throw new Error(
      "PROFILE returned invalid note_card, note_upgrades, or final_act_available",
    );
  }
  return profile;
}

function addCollectionMetadata(
  records,
  bossUnlocks,
  policySeed,
  gameSeed,
  startingHp,
  profile,
  sourceVersion = "working-tree",
  boundarySchema,
  { validateStateSchemas = true } = {},
) {
  if (boundarySchema !== REQUIRED_BOUNDARY_SCHEMA) {
    throw new Error(
      `CommunicationMod boundary_schema=${REQUIRED_BOUNDARY_SCHEMA} is required, received ${boundarySchema ?? "missing"}`,
    );
  }
  if (validateStateSchemas) {
    for (const state of records.filter((record) => record.type === "state")) {
      if (state.message?.boundary_schema !== boundarySchema) {
        throw new Error(
          `trace boundary_schema changed from ${boundarySchema} to ${state.message?.boundary_schema ?? "missing"}`,
        );
      }
    }
  }
  const metadata = {
    type: "metadata",
    schema: 1,
    boundary_schema: boundarySchema,
    source: "communication_mod",
    client: "simulator/tools/communication/random_fidelity_collector.js",
    source_version: sourceVersion,
    boss_unlocks: bossUnlocks,
    run_config: { profile },
    collection: { policy_seed: policySeed, game_seed: gameSeed, starting_hp: startingHp },
  };
  return [metadata, ...records.filter((record) => record.type !== "metadata")];
}

async function materializeRunEvidence({
  bridgeTracePath,
  runTraceStartOffset,
  destination,
  bossUnlocks,
  policySeed,
  gameSeed,
  startingHp,
  profile,
  sourceVersion,
  boundarySchema,
  settled,
}) {
  const extracted = await currentRunRecords(bridgeTracePath, runTraceStartOffset);
  const rawRecords = addCollectionMetadata(
    extracted,
    bossUnlocks,
    policySeed,
    gameSeed,
    startingHp,
    profile,
    sourceVersion,
    boundarySchema,
    { validateStateSchemas: settled },
  );
  const records = settled ? normalizeSettledGameplayRecords(rawRecords) : rawRecords;
  writeTrace(destination, records, { exclusive: true });
  return records;
}

function normalizeSettledGameplayRecords(records) {
  records = records.filter((record) =>
    ["metadata", "action", "state", "error", "external_rng"].includes(record.type),
  );
  for (const state of records.filter((record) => record.type === "state")) {
    communicationBoundary(state, { allowUnsettled: true });
  }

  const normalized = [];
  const seenCommandIds = new Set();
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index];
    if (record.type === "metadata") {
      normalized.push(record);
      continue;
    }
    if (record.type !== "action") {
      throw new Error(`orphan ${record.type} record at step ${record.step ?? "missing"}`);
    }
    if (!Number.isSafeInteger(record.step) || record.step < 0) {
      throw new Error("action step must be a non-negative integer");
    }
    const commandId = record.command_meta?.command_id;
    if (typeof commandId !== "string" || !commandId) {
      throw new Error(`schema-7 action at step ${record.step} requires command_id`);
    }
    if (seenCommandIds.has(commandId)) {
      throw new Error(`duplicate schema-7 command_id ${JSON.stringify(commandId)}`);
    }
    seenCommandIds.add(commandId);
    normalized.push(record);

    const stateCommand = String(record.command).trim().split(/\s+/)[0].toUpperCase() === "STATE";
    const sourceCommandExecutionSeq = record.command_meta?.source_command_execution_seq;
    const sourceCommandSettlementSeq = record.command_meta?.source_command_settlement_seq;
    let responseIndex = index + 1;
    let lastBoundaryKind = "missing";
    let completed = false;
    while (responseIndex < records.length) {
      const response = records[responseIndex];
      if (response.type === "metadata") {
        normalized.push(response);
        responseIndex += 1;
        continue;
      }
      if (response.type === "external_rng") {
        if (response.step !== record.step) {
          throw new Error(`external_rng step ${response.step} does not match action step ${record.step}`);
        }
        normalized.push(response);
        responseIndex += 1;
        continue;
      }
      if (response.type === "error") {
        if (response.step !== record.step) {
          throw new Error(`error step ${response.step} does not match action step ${record.step}`);
        }
        assertSchema7Rejection(record, response);
        normalized.push(response);
        index = responseIndex;
        completed = true;
        break;
      }
      if (response.type !== "state") break;
      if (response.step !== record.step) {
        throw new Error(`state step ${response.step} does not match action step ${record.step}`);
      }
      const boundary = communicationBoundary(response, { allowUnsettled: true });
      lastBoundaryKind = boundary.kind;
      if (
        !stateCommand
        && (!isSafeNonNegativeInteger(sourceCommandExecutionSeq)
          || !isSafeNonNegativeInteger(sourceCommandSettlementSeq)
          || typeof commandId !== "string"
          || !commandId)
      ) {
        throw new Error(
          `schema-7 gameplay action at step ${record.step} requires command_id and source execution/settlement sequences`,
        );
      }
      if (
        stateCommand
        && (typeof commandId !== "string"
          || !commandId
          || !isSafeNonNegativeInteger(sourceCommandExecutionSeq)
          || !isSafeNonNegativeInteger(sourceCommandSettlementSeq))
      ) {
        throw new Error(
          `schema-7 STATE action at step ${record.step} requires command_id and source execution/settlement sequences`,
        );
      }
      const commandFenceAdvanced = stateCommand
        ? response.message.command_execution_seq === sourceCommandExecutionSeq
          && response.message.command_settlement_seq === sourceCommandSettlementSeq
        : response.message.command_execution_seq === sourceCommandExecutionSeq + 1
          && response.message.command_settlement_seq === sourceCommandSettlementSeq + 1;
      const identityMatches = response.message.command_response_id === commandId;
      const completesCommand = stateCommand
        ? boundary.kind === "poll"
          && response.message.command_response_kind === "poll"
          && identityMatches
          && commandFenceAdvanced
        : GAMEPLAY_BOUNDARY_KINDS.has(boundary.kind)
          && response.message.command_response_kind === "settled"
          && identityMatches
          && commandFenceAdvanced;
      if (!completesCommand) {
        if (response.message.command_response_kind === "unsolicited") {
          responseIndex += 1;
          continue;
        }
        if (
          !stateCommand
          && GAMEPLAY_BOUNDARY_KINDS.has(boundary.kind)
          && !commandFenceAdvanced
        ) {
          // The game had not executed this command yet. Preserve the evidence
          // in the raw bridge trace, but omit the overtaking state from the
          // strict one-action/one-completion corpus payload.
          responseIndex += 1;
          continue;
        }
        // Under SuperFastMode, a STATE poll can be followed by a same-step
        // gameplay settlement frame before the poll marker is observed. Keep
        // scanning for the authoritative completing boundary; do not keep the
        // intermediate frame in the normalized trace.
        if (
          stateCommand &&
          (GAMEPLAY_BOUNDARY_KINDS.has(boundary.kind) || boundary.kind === "unknown")
        ) {
          responseIndex += 1;
          continue;
        }
        if (
          !stateCommand &&
          (boundary.kind === "poll" || boundary.kind === "unknown")
        ) {
          responseIndex += 1;
          continue;
        }
        const expected = stateCommand
          ? "poll"
          : "interaction_ready, quiescent, or terminal";
        throw new Error(
          `${record.command} completed on ${boundary.kind}; expected ${expected}`,
        );
      }
      normalized.push(response);
      index = responseIndex;
      completed = true;
      break;
    }
    if (!completed) {
      const commandKind = stateCommand ? "STATE" : "gameplay";
      throw new Error(
        `${commandKind} action at step ${record.step} produced ${lastBoundaryKind}, not a completing boundary`,
      );
    }
  }
  return normalized;
}


async function currentRunRecords(tracePath, startOffset = 0) {
  const input = fs.createReadStream(tracePath, {
    encoding: "utf8",
    start: startOffset,
  });
  const lines = readline.createInterface({ input, crlfDelay: Infinity });
  let latestRun = null;
  let previous = null;
  let lineNumber = 0;

  try {
    for await (const line of lines) {
      lineNumber += 1;
      if (!line.trim()) continue;
      const isStartLine =
        line.includes('"type":"action"') &&
        /"command":"START(?:_VERIFY)?\s+/i.test(line);
      if (!latestRun && !isStartLine) {
        previous = line;
        continue;
      }

      let record;
      try {
        record = JSON.parse(line);
      } catch (error) {
        throw new Error(`${tracePath}:${lineNumber}: ${error.message}`);
      }

      if (isStartLine) {
        let preamble = null;
        if (
          previous?.startsWith('{"type":"state"') &&
          (previous.includes('"in_game":false') || previous.includes('"screen_type":"MAIN_MENU"'))
        ) {
          try {
            preamble = JSON.parse(previous);
          } catch (error) {
            throw new Error(`${tracePath}:${lineNumber - 1}: ${error.message}`);
          }
        }
        latestRun = preamble ? [preamble, record] : [record];
      } else if (latestRun) {
        latestRun.push(record);
      }
      previous = line;
    }
  } finally {
    lines.close();
  }

  if (!latestRun) throw new Error("bridge trace contains no started run");
  const startStep = latestRun.find((record) => record.type === "action").step;
  const stepOffset = startStep - 1;
  const extracted = latestRun.map((record) => {
    const copy = { ...record };
    if (typeof copy.step === "number") copy.step -= stepOffset;
    return copy;
  });
  extracted.unshift({
    type: "metadata",
    schema: 1,
    source: "communication_mod",
    event: "extracted_run",
    source_trace: path.basename(tracePath),
    source_run_index: null,
    source_start_step: startStep,
    created_at: new Date().toISOString(),
  });
  return extracted;
}

function requireCommandCompletion(protocolState, command, acceptedStep, priorStateSeq) {
  const current = protocolState?.state ?? protocolState;
  if (!current || current.ok === false) {
    throw new Error(`bridge did not return a completion for ${command}`);
  }
  if (current.step !== acceptedStep) {
    throw new Error(`bridge completion step ${current.step} does not match action step ${acceptedStep}`);
  }
  if (!Number.isSafeInteger(current.state_seq) || current.state_seq <= priorStateSeq) {
    throw new Error(`bridge completion for ${command} did not advance state_seq`);
  }
  const summary = current.summary;
  if (summary?.error) return current;
  const commandHead = String(command).trim().split(/\s+/)[0].toUpperCase();
  if (commandHead === "PROFILE") {
    if (summary?.type !== "profile" || typeof summary.profile !== "object" || summary.profile === null) {
      throw new Error("PROFILE did not return typed profile metadata");
    }
    return current;
  }
  const boundary = communicationBoundary(summary);
  const stateCommand = commandHead === "STATE";
  const valid = stateCommand
    ? boundary.kind === "poll"
    : GAMEPLAY_BOUNDARY_KINDS.has(boundary.kind);
  if (!valid) {
    throw new Error(`${command} completed on invalid ${boundary.kind} boundary`);
  }
  return current;
}

async function send(control, ownerToken, protocolState, command, metadata) {
  const playtimeSeconds = protocolState.summary?.playtime_seconds;
  if (Number.isFinite(playtimeSeconds) && playtimeSeconds >= 0) {
    metadata = { ...metadata, playtime_seconds: Math.floor(playtimeSeconds) };
  }
  const response = await controlRequest(
    control,
    {
      type: "command",
      command,
      command_id: crypto.randomUUID(),
      expected_state_id: protocolState.state_id,
      expected_state_seq: protocolState.state_seq,
      owner_token: ownerToken,
      metadata,
      wait_for_state_update: true,
      update_timeout_ms: 20000,
    },
    25000,
  );
  if (!response.ok) throw new Error(response.error || `bridge rejected ${command}`);
  const acceptedActionStep = response.step + (protocolState.summary ? 1 : 0);
  if (response.observed_update?.ok) {
    return requireCommandCompletion(
      response.observed_update,
      command,
      acceptedActionStep,
      protocolState.state_seq,
    );
  }

  // A long animation can outlive update_timeout_ms even though the bridge has
  // accepted and queued the command. Do not submit a settle STATE into that
  // occupied slot; wait for the accepted command to finish instead.
  for (let attempt = 0; attempt < 240; attempt += 1) {
    const current = await controlRequest(control, { type: "state" });
    if (current.ok && !current.pending_command && current.state_seq > protocolState.state_seq) {
      return requireCommandCompletion(current, command, acceptedActionStep, protocolState.state_seq);
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  const hang = new Error(`bridge command did not complete after acceptance: ${command}`);
  hang.code = "COMMAND_IN_FLIGHT_HANG";
  hang.command = command;
  throw hang;
}

async function main() {
  const sessionDir = path.resolve(process.env.STS_BRIDGE_SESSION_DIR || defaultSessionDir);
  const status = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
  if (!status.control || status.control.protocol !== "tcp-jsonl") {
    throw new Error("a live TCP-enabled CommunicationMod bridge is required");
  }
  const policySeed = Number.parseInt(process.env.STS_RANDOM_POLICY_SEED || "1", 10);
  const gameSeed = process.env.STS_GAME_SEED || `COLLECT${policySeed}`;
  const startingHp = Number.parseInt(process.env.STS_STARTING_HP || "10000", 10);
  const maxActions = Number.parseInt(process.env.STS_RANDOM_MAX_ACTIONS || "10000", 10);
  const logActions = process.env.STS_RANDOM_LOG_ACTIONS !== "0";
  const sourceVersion = process.env.STS_RANDOM_SOURCE_VERSION || "working-tree";
  const traceStartedAt = new Date();
  // Snapshot this immediately before starting the run: the game mutates this
  // profile-scoped preference as previously unseen bosses are encountered.
  const bossUnlocks = loadBossUnlocks();
  const random = seededRandom(policySeed);
  const ownerId = `random-fidelity-${process.pid}`;
  const acquired = await controlRequest(status.control, {
    type: "acquire",
    owner_id: ownerId,
    takeover_if_stale_after_ms: 5000,
    cancel_orphaned_command_after_ms: 5000,
  });
  if (!acquired.ok) throw new Error(acquired.error || "bridge ownership rejected");

  const campaignDir = path.resolve(
    process.env.STS_RANDOM_OUTPUT_DIR || path.join(root, "target", "random-fidelity"),
  );
  fs.mkdirSync(campaignDir, { recursive: true });
  const activeTrace = immutableTracePath(campaignDir, gameSeed, policySeed, traceStartedAt);
  const ledgerPath = path.join(campaignDir, "ledger.jsonl");

  const abandonCurrentRun = async ({ reason, preemptInFlight = false }) => {
    const current = await controlRequest(status.control, { type: "state" });
    if (!current.summary?.in_game) return;
    const abandoned = await controlRequest(
      status.control,
      {
        type: "abandon_run",
        owner_token: acquired.owner_token,
        preempt_in_flight: preemptInFlight || undefined,
        metadata: {
          source: "random_fidelity_collector",
          reason: "captured_incomplete_run",
          detail: reason,
        },
        wait_for_state_update: true,
        update_timeout_ms: 20000,
      },
      25000,
    );
    if (!abandoned.ok) {
      throw new Error(abandoned.error || `failed to abandon captured run: ${reason}`);
    }
  };

  try {
    let protocolState = await controlRequest(status.control, { type: "state" });
    if (!protocolState.ok) throw new Error(protocolState.error || "failed to read bridge state");
    if (protocolState.summary?.in_game) {
      if (process.env.STS_RANDOM_ABANDON_EXISTING !== "1") {
        throw new Error("bridge is already in a run; abandon it or set STS_RANDOM_ABANDON_EXISTING=1");
      }
      const abandoned = await controlRequest(
        status.control,
        {
          type: "abandon_run",
          owner_token: acquired.owner_token,
          metadata: { source: "random_fidelity_collector", reason: "next_campaign_run" },
          wait_for_state_update: true,
          update_timeout_ms: 20000,
        },
        25000,
      );
      if (!abandoned.ok) throw new Error(abandoned.error || "failed to abandon existing run");
      await new Promise((resolve) => setTimeout(resolve, 3000));
      for (let attempt = 0; attempt < 20; attempt += 1) {
        protocolState = await controlRequest(status.control, { type: "state" });
        if (!protocolState.summary?.in_game) break;
        if (protocolState.summary?.ready_for_command) {
          await send(status.control, acquired.owner_token, protocolState, "STATE", {
            source: "random_fidelity_collector",
            reason: "settle_abandon",
            operator_control: "settle_poll",
          });
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
      }
      if (protocolState.summary?.in_game) throw new Error("game did not return to menu after abandon");
    }
    const startupReady = !protocolState.summary && protocolState.status?.status === "ready";
    if (startupReady) {
      await send(status.control, acquired.owner_token, protocolState, "STATE", {
        source: "random_fidelity_collector",
        reason: "observe_startup_menu",
        operator_control: "startup_state",
      });
      protocolState = await controlRequest(status.control, { type: "state" });
    }
    for (let attempt = 0; attempt < 240; attempt += 1) {
      if (menuStartReady(protocolState.summary)) break;
      if (protocolState.summary || protocolState.status?.status === "ready") {
        await send(status.control, acquired.owner_token, protocolState, "STATE", {
          source: "random_fidelity_collector",
          reason: "settle_main_menu",
          operator_control: "settle_poll",
        });
      }
      await new Promise((resolve) => setTimeout(resolve, 500));
      protocolState = await controlRequest(status.control, { type: "state" });
    }
    if (!menuStartReady(protocolState.summary)) {
      throw new Error("START_VERIFY did not become available at the main menu");
    }
    const profileState = await send(
      status.control,
      acquired.owner_token,
      protocolState,
      "PROFILE",
      { source: "random_fidelity_collector", reason: "capture_pre_run_profile" },
    );
    const profile = validateProfileSnapshot(profileState.summary?.profile);
    protocolState = await controlRequest(status.control, { type: "state" });
    if (!protocolState.ok || !availableSet(protocolState.summary).has("start_verify")) {
      throw new Error("bridge did not restore the command boundary after PROFILE");
    }

    // The bridge-level trace is intentionally append-only for the lifetime of
    // CommunicationMod. Remember this run's byte boundary so verification does
    // not rescan every prior campaign run (or an old multi-gigabyte prefix).
    const bridgeTracePath = localBridgeTracePath(protocolState.trace_path, sessionDir);
    const runTraceStartOffset = fs.statSync(bridgeTracePath).size;
    const boundarySchema = communicationBoundary(protocolState).message.boundary_schema;
    const start = `START_VERIFY IRONCLAD 0 ${gameSeed} ${startingHp}`;

    let traceRecorded = false;
    const materializeRun = async ({ terminalReason, reason = null, actions, settled }) => {
      if (traceRecorded) return;
      await materializeRunEvidence({
        bridgeTracePath,
        runTraceStartOffset,
        destination: activeTrace,
        bossUnlocks,
        policySeed,
        gameSeed,
        startingHp,
        profile,
        sourceVersion,
        boundarySchema,
        settled,
      });
      traceRecorded = true;
      const entry = {
        recorded_at: new Date().toISOString(),
        kind: settled ? "collected" : "collected_incomplete",
        game_seed: gameSeed,
        policy_seed: policySeed,
        starting_hp: startingHp,
        actions,
        terminal_reason: terminalReason,
        reason,
        trace: activeTrace,
      };
      fs.appendFileSync(ledgerPath, `${JSON.stringify(entry)}\n`);
      console.log(JSON.stringify(entry));
    };

    let runObserved = false;
    let pendingEventLeave = null;
    let attemptedActions = 0;
    try {
      await send(status.control, acquired.owner_token, protocolState, start, {
        source: "random_fidelity_collector",
        policy_seed: policySeed,
        game_seed: gameSeed,
        action_index: 0,
      });

      // The extra sentinel iteration settles and records the final commanded
      // action before a max-actions result is reported.
      for (let actionIndex = 1; actionIndex <= maxActions + 1; actionIndex += 1) {
      protocolState = await controlRequest(status.control, { type: "state" });
      const summary = protocolState.summary;
      if (!summary) throw new Error("bridge returned no summary");
      if (pendingEventLeave && isSoleEventLeaveScreen(summary)) {
        const confirmEventLeave = ["confirm", "confirm_fold"].includes(pendingEventLeave);
        const settleCommand = confirmEventLeave ? "CHOOSE 0" : "STATE";
        await send(status.control, acquired.owner_token, protocolState, settleCommand, {
          source: "random_fidelity_collector",
          policy_seed: policySeed,
          game_seed: gameSeed,
          action_index: actionIndex,
          operator_control: "settle_gameplay",
          reason:
            confirmEventLeave
              ? pendingEventLeave === "confirm_fold"
                ? "confirm_selected_event_leave"
                : "confirm_event_leave"
              : "settle_event_leave",
        });
        pendingEventLeave = "poll";
        actionIndex -= 1;
        continue;
      }
      if (pendingEventLeave) pendingEventLeave = false;
      if (!summary.in_game && !runObserved) {
        await send(status.control, acquired.owner_token, protocolState, "STATE", {
          source: "random_fidelity_collector",
          policy_seed: policySeed,
          game_seed: gameSeed,
          action_index: actionIndex,
          operator_control: "settle_start",
        });
        await new Promise((resolve) => setTimeout(resolve, 200));
        actionIndex -= 1;
        continue;
      }
      if (summary.in_game) runObserved = true;
      if (!summary.ready_for_command) {
        await send(status.control, acquired.owner_token, protocolState, "STATE", {
          source: "random_fidelity_collector",
          policy_seed: policySeed,
          game_seed: gameSeed,
          action_index: actionIndex,
          operator_control: "settle_poll",
        });
        actionIndex -= 1;
        continue;
      }
      if (needsMapChoiceSettle(summary)) {
        await send(status.control, acquired.owner_token, protocolState, "STATE", {
          source: "random_fidelity_collector",
          policy_seed: policySeed,
          game_seed: gameSeed,
          action_index: actionIndex,
          operator_control: "settle_map_choices",
        });
        actionIndex -= 1;
        continue;
      }
      const actionCount = actionIndex - 1;
      const terminal = !summary.in_game || actionIndex > maxActions;
      if (terminal) {
        await materializeRun({
          terminalReason: !summary.in_game ? "game_over" : "max_actions",
          actions: actionCount,
          settled: true,
        });
        return;
      }
      const sample = sampleRandomAction(summary, random);
      const command = sample?.command ?? null;
      if (!command) {
        throw new Error(
          `no concrete gameplay action for screen=${summary.screen_type} available=${JSON.stringify(summary.available_commands)}`,
        );
      }
      if (logActions) console.log(`[${actionIndex}] ${command}`);
      const eventChoice =
        String(summary.screen_type).toUpperCase() === "EVENT" && command.startsWith("CHOOSE ");
      const neowEvent = String(summary.room_type).toLowerCase() === "neowroom";
      const directSettleEvent = ["goldenidol", "upgradeshrine"].includes(
        String(summary.event_id).toLowerCase().replaceAll(" ", ""),
      );
      const selectedEventChoice = eventChoice
        ? summary.choices?.[Number.parseInt(command.slice("CHOOSE ".length), 10)]
        : null;
      const selectedEventLeave = String(selectedEventChoice).toLowerCase() === "leave";
      const eventLeaveMode = eventChoice
        ? isSoleEventLeaveScreen(summary)
          ? "poll"
          : neowEvent || !directSettleEvent
            ? null
            : selectedEventLeave
              ? "confirm_fold"
              : "confirm"
        : null;
      attemptedActions = actionIndex;
      await send(status.control, acquired.owner_token, protocolState, command, {
        source: "random_fidelity_collector",
        policy_seed: policySeed,
        game_seed: gameSeed,
        action_index: actionIndex,
        action_count: sample.actions.length,
        eligible_actions: sample.actions,
        selected_action_index: sample.selectedIndex,
      });
      pendingEventLeave = eventLeaveMode;
      }
    } catch (error) {
      const reason = error.stack || error.message || String(error);
      if (!traceRecorded) {
        await materializeRun({
          terminalReason: isCommandInFlightHang(error) ? "command_hang" : "collector_error",
          reason,
          actions: attemptedActions,
          settled: false,
        });
      }
      await abandonCurrentRun({
        reason,
        preemptInFlight: isCommandInFlightHang(error),
      }).catch((abandonError) => {
        console.error(`captured run but failed to reset game: ${abandonError.message || abandonError}`);
      });
      return;
    }
  } finally {
    await controlRequest(status.control, {
      type: "release",
      owner_token: acquired.owner_token,
    }).catch(() => {});
  }
}

if (require.main === module) {
  main().catch((error) => {
    const detail = error.stack || error.message || String(error);
    console.error(detail);
    try {
      const errorPath = path.join(
        path.resolve(
          process.env.STS_RANDOM_OUTPUT_DIR || path.join(root, "target", "random-fidelity"),
        ),
        "errors.log",
      );
      fs.mkdirSync(path.dirname(errorPath), { recursive: true });
      fs.appendFileSync(errorPath, `[${new Date().toISOString()}] ${detail}\n`);
    } catch {}
    process.exit(1);
  });
}

module.exports = {
  addCollectionMetadata,
  chooseRandomAction,
  currentRunRecords,
  enumerateGameplayActions,
  immutableTracePath,
  isCommandInFlightHang,
  loadBossUnlocks,
  localBridgeTracePath,
  isSoleEventLeaveScreen,
  communicationBoundary,
  materializeRunEvidence,
  menuStartReady,
  needsMapChoiceSettle,
  normalizeSettledGameplayRecords,
  parseBossUnlocks,
  parseSeenBossesPreferences,
  sampleRandomAction,
  seededRandom,
  validateProfileSnapshot,
  writeTrace,
};
