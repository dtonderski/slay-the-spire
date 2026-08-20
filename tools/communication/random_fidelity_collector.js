#!/usr/bin/env node

const crypto = require("crypto");
const fs = require("fs");
const net = require("net");
const path = require("path");
const readline = require("readline");

const root = path.resolve(__dirname, "..", "..");
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

function livingMonsters(summary) {
  return (summary?.combat?.monsters || []).filter(
    (monster) => !monster.gone && !monster.half_dead && Number(monster.hp) > 0,
  );
}

function totalLivingEnemyHp(summary) {
  return livingMonsters(summary).reduce((sum, monster) => sum + Number(monster.hp || 0), 0);
}

/** Player strength modifier from the combat summary. Missing power => 0. */
function playerCombatStrength(summary) {
  const direct = summary?.combat?.player_strength;
  if (Number.isFinite(Number(direct))) return Number(direct);
  return 0;
}

function createCombatStallTracker() {
  return {
    lastCombatTurn: null,
    lastEnemyHp: null,
    stagnantTurns: 0,
  };
}

/**
 * Track multi-turn no-damage stalls while player strength is deeply negative.
 * Abandon when strength < -10 and living enemy HP is unchanged across 3 turn advances.
 */
function observeCombatStall(tracker, summary) {
  if (!tracker) {
    return { shouldAbandon: false, reason: null };
  }
  if (!summary?.in_game || !summary?.combat) {
    tracker.lastCombatTurn = null;
    tracker.lastEnemyHp = null;
    tracker.stagnantTurns = 0;
    return { shouldAbandon: false, reason: null };
  }

  const strength = playerCombatStrength(summary);
  const enemyHp = totalLivingEnemyHp(summary);
  const turn = Number(summary.combat.turn);
  if (!Number.isFinite(turn)) {
    return { shouldAbandon: false, reason: null };
  }

  if (strength >= -10) {
    tracker.stagnantTurns = 0;
    tracker.lastCombatTurn = turn;
    tracker.lastEnemyHp = enemyHp;
    return { shouldAbandon: false, reason: null };
  }

  if (tracker.lastCombatTurn === null) {
    tracker.lastCombatTurn = turn;
    tracker.lastEnemyHp = enemyHp;
    tracker.stagnantTurns = 0;
    return { shouldAbandon: false, reason: null };
  }

  if (turn !== tracker.lastCombatTurn) {
    if (enemyHp === tracker.lastEnemyHp) {
      tracker.stagnantTurns += 1;
    } else {
      tracker.stagnantTurns = 0;
      tracker.lastEnemyHp = enemyHp;
    }
    tracker.lastCombatTurn = turn;
  } else if (enemyHp !== tracker.lastEnemyHp) {
    tracker.stagnantTurns = 0;
    tracker.lastEnemyHp = enemyHp;
  }

  if (tracker.stagnantTurns >= 3) {
    return {
      shouldAbandon: true,
      reason: `combat stall: player strength ${strength} < -10 and enemy HP unchanged for ${tracker.stagnantTurns} turns (hp=${enemyHp})`,
    };
  }
  return { shouldAbandon: false, reason: null };
}

function enumerateGameplayActions(summary) {
  const available = availableSet(summary);
  const actions = [];
  const choices = Array.isArray(summary?.choices) ? summary.choices : [];
  const shopPotionNames = new Set(
    (summary?.shop_potions || []).map((potion) => String(potion?.name).toLowerCase()),
  );

  if (available.has("choose")) {
    const normalizedChoices = choices.map((choice) => String(choice).toLowerCase());
    const settleStolenGoldOrderFirst =
      normalizedChoices.includes("stolen_gold") && normalizedChoices.includes("gold");
    const avoidKnownGoopDamage =
      normalizedChoices.includes("gather gold") && normalizedChoices.includes("leave it");
    const avoidKnownFaceTrade =
      normalizedChoices.includes("touch") &&
      normalizedChoices.includes("trade") &&
      normalizedChoices.includes("leave");
    choices.forEach((choice, index) => {
      const normalizedChoice = String(choice).toLowerCase();
      if (
        String(summary?.screen_type).toUpperCase() === "SHOP_SCREEN" &&
        (normalizedChoice.includes("potion") || shopPotionNames.has(normalizedChoice))
      ) {
        // CommunicationMod accepts shop potion purchases but does not expose
        // the resulting potion-selection/settlement flow to strict replay.
        // Preserve those traces when encountered, but avoid hanging the
        // campaign on a command that never yields a settled state.
        return;
      }
      if (
        settleStolenGoldOrderFirst &&
        normalizedChoice !== "stolen_gold" &&
        normalizedChoice !== "gold"
      ) {
        return;
      }
      if (avoidKnownGoopDamage && normalizedChoice === "gather gold") return;
      if (avoidKnownFaceTrade && normalizedChoice === "trade") return;
      if (
        normalizedChoice === "potion" &&
        String(summary?.screen_type).toUpperCase() === "COMBAT_REWARD"
      ) {
        // Potion rewards can deadlock CommunicationMod even when the preceding
        // snapshot shows an open slot: potion effects such as Entropic Brew
        // may refill that slot after the snapshot but before CHOOSE executes.
        // Collect another reward instead of risking a silent accepted command.
        return;
      }
      if (
        String(summary?.room_type).toLowerCase() === "neowroom" &&
        normalizedChoice.includes("obtain 3 random potions")
      ) {
        // The seed-start bootstrap cannot replay the subsequent three-potion
        // reward screen. This independently blocked multiple runs at step 5.
        return;
      }
      actions.push(`CHOOSE ${index}`);
    });
  }
  if (available.has("play")) {
    const monsters = livingMonsters(summary);
    for (const card of summary?.combat?.hand || []) {
      if (!card.playable) continue;
      if (String(card.id).toLowerCase() === "armaments") {
        // CommunicationMod snapshots Armaments after the selected card leaves
        // the grid but before it returns upgraded to the hand. Repeated traces
        // confirm that this transient cannot be strict replay evidence.
        continue;
      }
      if (String(card.id).toLowerCase() === "dual wield") {
        // CommunicationMod snapshots the auto-confirm grid before Dual Wield
        // returns the selected card and its copies to hand, so strict replay
        // cannot compare a settled post-action state.
        continue;
      }
      if (String(card.id).toLowerCase() === "exhume") {
        // Exhume's grid auto-confirms before the selected exhaust card has
        // finished moving, producing the same unsettled selection evidence as
        // other excluded card-selection actions.
        continue;
      }
      if (String(card.id).toLowerCase() === "true grit") {
        // Base True Grit's random exhaust and upgraded True Grit's selection
        // settlement have both produced independent permanent divergences.
        // Preserve those witnesses without repeatedly terminating later runs.
        continue;
      }
      if (String(card.id).toLowerCase() === "sword boomerang") {
        // Its random hit targeting has independently diverged more than once.
        // Keep the original permanent trace, but skip repeated instances so
        // the campaign can continue into later rooms.
        continue;
      }
      if (
        String(card.id).toLowerCase() === "headbutt" &&
        Number(summary?.combat?.discard_pile_count || 0) > 1
      ) {
        // CommunicationMod pauses after the grid closes but before Headbutt's
        // queued discard-to-draw move. No polling command can expose a settled
        // observation, so this action cannot currently produce strict evidence.
        continue;
      }
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
      if (!potion.can_use) continue;
      const potionId = String(potion.id).toLowerCase();
      if (potionId === "liquidmemories") {
        // Like Headbutt's grid, CommunicationMod observes this auto-confirm
        // before the selected discard card has moved into the hand.
        continue;
      }
      if (potionId === "gamblersbrew") {
        // The multi-card discard screen closes before the redraw queue has
        // settled, so the observed post-CONFIRM hand can precede the real draw.
        continue;
      }
      if (["attackpotion", "skillpotion", "powerpotion", "colorlesspotion"].includes(potionId)) {
        // Card-generating potions have now produced repeated divergences in the
        // generated-card reward/selection flow. Preserve those traces, but do
        // not let the same unsupported family terminate future coverage runs.
        continue;
      }
      if (potion.requires_target) {
        monsters.forEach((monster) => actions.push(`POTION USE ${potion.index} ${monster.index}`));
      } else {
        actions.push(`POTION USE ${potion.index}`);
      }
    }
  }
  if (
    available.has("key") &&
    String(summary?.screen_type).toUpperCase() === "NONE" &&
    String(summary?.screen_name).toUpperCase() === "MASTER_DECK_VIEW" &&
    String(summary?.room_type).toLowerCase() === "treasureroomboss"
  ) {
    // Choosing a boss relic opens the master-deck overlay. CommunicationMod
    // exposes its dismissal as KEY CANCEL, and the seed-start verifier has a
    // dedicated replay path for the settled post-overlay state.
    actions.push("KEY CANCEL");
  }
  for (const [verb, command] of [
    ["end", "END"],
    ["proceed", "PROCEED"],
    ["return", "RETURN"],
    ["confirm", "CONFIRM"],
    ["skip", "SKIP"],
    ["leave", "LEAVE"],
  ]) {
    if (!available.has(verb)) continue;
    if (command === "RETURN" && actions.some((action) => action.startsWith("CHOOSE "))) {
      // On an ordinary generated map, choose a node. RETURN is only needed for
      // overlays that expose no concrete map choice, and seed-start replay
      // cannot consume a choose/return detour.
      continue;
    }
    if (
      command === "SKIP" &&
      String(summary?.room_type).toLowerCase() === "neowroom" &&
      String(summary?.screen_type).toUpperCase() === "CARD_REWARD"
    ) {
      // The seed-start bootstrap can replay a Neow colorless pick but not a
      // SKIP transition from that reward screen. This independently stopped
      // multiple runs at the first reward action.
      continue;
    }
    if (
      command === "PROCEED" &&
      String(summary?.screen_type).toUpperCase() === "COMBAT_REWARD" &&
      actions.some((action) => action.startsWith("CHOOSE "))
    ) {
      // Leaving claimable rewards opens a MAP overlay whose RETURN detour is
      // not replayable by the seed-start verifier. Resolve rewards first.
      continue;
    }
    actions.push(command);
  }
  return [...new Set(actions)];
}

function chooseRandomAction(summary, random) {
  const actions = enumerateGameplayActions(summary);
  if (actions.length === 0) return null;
  return actions[Math.floor(random() * actions.length)];
}

const GAMEPLAY_BOUNDARY_KINDS = new Set(["interaction_ready", "quiescent", "terminal"]);
const REQUIRED_BOUNDARY_SCHEMA = 6;

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
  if (!Number.isInteger(message?.command_execution_seq) || message.command_execution_seq < 0) {
    throw new Error(
      `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires a non-negative integer command_execution_seq`,
    );
  }
  for (const field of [
    "effects_size",
    "top_level_effects_size",
    "queued_top_level_effects_size",
  ]) {
    if (!Number.isInteger(message?.[field]) || message[field] !== 0) {
      throw new Error(
        `CommunicationMod schema ${REQUIRED_BOUNDARY_SCHEMA} requires ${field}=0`,
      );
    }
  }
  if (GAMEPLAY_BOUNDARY_KINDS.has(kind)) {
    if (message?.ready_for_command !== true) {
      throw new Error(`${kind} CommunicationMod boundary is not ready for input`);
    }
    if (message.end_turn_queued && kind !== "interaction_ready") {
      throw new Error(`${kind} CommunicationMod boundary cannot have an end turn queued`);
    }
  }
  return { kind, message };
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
  const descriptor = fs.openSync(filePath, exclusive ? "wx" : "w");
  try {
    for (const record of records) {
      fs.writeSync(descriptor, `${JSON.stringify(record)}\n`);
    }
  } finally {
    fs.closeSync(descriptor);
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
) {
  if (boundarySchema !== REQUIRED_BOUNDARY_SCHEMA) {
    throw new Error(
      `CommunicationMod boundary_schema=${REQUIRED_BOUNDARY_SCHEMA} is required, received ${boundarySchema ?? "missing"}`,
    );
  }
  for (const state of records.filter((record) => record.type === "state")) {
    if (state.message?.boundary_schema !== boundarySchema) {
      throw new Error(
        `trace boundary_schema changed from ${boundarySchema} to ${state.message?.boundary_schema ?? "missing"}`,
      );
    }
  }
  const metadata = {
    type: "metadata",
    schema: 1,
    boundary_schema: boundarySchema,
    source: "communication_mod",
    client: "tools/communication/random_fidelity_collector.js",
    source_version: sourceVersion,
    boss_unlocks: bossUnlocks,
    run_config: { profile },
    collection: { policy_seed: policySeed, game_seed: gameSeed, starting_hp: startingHp },
  };
  return [metadata, ...records.filter((record) => record.type !== "metadata")];
}

function normalizeSettledGameplayRecords(records) {
  records = records.filter((record) =>
    ["metadata", "action", "state", "error", "external_rng"].includes(record.type),
  );
  for (const state of records.filter((record) => record.type === "state")) {
    communicationBoundary(state, { allowUnsettled: true });
  }

  const normalized = [];
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
    normalized.push(record);

    const stateCommand = String(record.command).trim().split(/\s+/)[0].toUpperCase() === "STATE";
    const sourceCommandExecutionSeq = record.command_meta?.source_command_execution_seq;
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
        && (!Number.isInteger(sourceCommandExecutionSeq) || sourceCommandExecutionSeq < 0)
      ) {
        throw new Error(
          `schema-5 gameplay action at step ${record.step} requires source_command_execution_seq`,
        );
      }
      const commandFenceAdvanced = stateCommand
        || response.message.command_execution_seq > sourceCommandExecutionSeq;
      const completesCommand = stateCommand
        ? boundary.kind === "poll"
        : GAMEPLAY_BOUNDARY_KINDS.has(boundary.kind) && commandFenceAdvanced;
      if (!completesCommand) {
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
  throw new Error(`bridge command did not complete after acceptance: ${command}`);
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
    process.env.STS_RANDOM_OUTPUT_DIR || path.join(root, "simulator", "target", "random-fidelity"),
  );
  fs.mkdirSync(campaignDir, { recursive: true });
  const activeTrace = immutableTracePath(campaignDir, gameSeed, policySeed, traceStartedAt);
  const ledgerPath = path.join(campaignDir, "ledger.jsonl");
  const skippedSeedsPath = path.join(campaignDir, "skipped_policy_seeds.jsonl");
  const seedPrefix = process.env.STS_RANDOM_GAME_SEED_PREFIX || "FIDL";
  const combatStallTracker = createCombatStallTracker();

  const discardActiveTrace = () => {
    try {
      if (fs.existsSync(activeTrace)) fs.unlinkSync(activeTrace);
    } catch {
      // best-effort: stall traces must not remain as corpus evidence
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
    for (let attempt = 0; attempt < 50; attempt += 1) {
      const available = availableSet(protocolState.summary);
      if (available.has("start") || available.has("start_verify")) break;
      await send(status.control, acquired.owner_token, protocolState, "STATE", {
        source: "random_fidelity_collector",
        reason: "settle_main_menu",
        operator_control: "settle_poll",
      });
      await new Promise((resolve) => setTimeout(resolve, 200));
      protocolState = await controlRequest(status.control, { type: "state" });
    }
    if (!availableSet(protocolState.summary).has("start") &&
        !availableSet(protocolState.summary).has("start_verify")) {
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
    const start = `START_VERIFY IRONCLAD 0 ${gameSeed} ${startingHp}`;
    await send(status.control, acquired.owner_token, protocolState, start, {
      source: "random_fidelity_collector",
      policy_seed: policySeed,
      game_seed: gameSeed,
      action_index: 0,
    });

    let runObserved = false;
    let pendingEventLeave = null;
    // The extra sentinel iteration settles and verifies the final commanded
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
        const rawRecords = addCollectionMetadata(
          await currentRunRecords(bridgeTracePath, runTraceStartOffset),
          bossUnlocks,
          policySeed,
          gameSeed,
          startingHp,
          profile,
          sourceVersion,
          communicationBoundary(protocolState).message.boundary_schema,
        );
        const records = normalizeSettledGameplayRecords(rawRecords);
        writeTrace(activeTrace, records, { exclusive: true });
        const entry = {
          recorded_at: new Date().toISOString(),
          kind: "collected",
          game_seed: gameSeed,
          policy_seed: policySeed,
          starting_hp: startingHp,
          actions: actionCount,
          terminal_reason: !summary.in_game ? "game_over" : "max_actions",
          trace: activeTrace,
        };
        fs.appendFileSync(ledgerPath, `${JSON.stringify(entry)}\n`);
        console.log(JSON.stringify(entry));
        return;
      }
      const stall = observeCombatStall(combatStallTracker, summary);
      if (stall.shouldAbandon) {
        const abandoned = await controlRequest(
          status.control,
          {
            type: "abandon_run",
            owner_token: acquired.owner_token,
            metadata: {
              source: "random_fidelity_collector",
              reason: "combat_stall",
              detail: stall.reason,
            },
            wait_for_state_update: true,
            update_timeout_ms: 20000,
          },
          25000,
        );
        if (!abandoned.ok) {
          throw new Error(abandoned.error || `failed to abandon stalled combat: ${stall.reason}`);
        }
        await new Promise((resolve) => setTimeout(resolve, 1000));
        for (let attempt = 0; attempt < 20; attempt += 1) {
          protocolState = await controlRequest(status.control, { type: "state" });
          if (!protocolState.summary?.in_game) break;
          if (protocolState.summary?.ready_for_command) {
            await send(status.control, acquired.owner_token, protocolState, "STATE", {
              source: "random_fidelity_collector",
              reason: "settle_combat_stall_abandon",
              operator_control: "settle_poll",
            });
          } else {
            await new Promise((resolve) => setTimeout(resolve, 250));
          }
        }
        discardActiveTrace();
        const entry = {
          recorded_at: new Date().toISOString(),
          kind: "discarded_combat_stall",
          game_seed: gameSeed,
          policy_seed: policySeed,
          starting_hp: startingHp,
          actions: actionCount,
          reason: stall.reason,
          trace: null,
        };
        fs.appendFileSync(ledgerPath, `${JSON.stringify(entry)}\n`);
        fs.appendFileSync(
          skippedSeedsPath,
          `${JSON.stringify({
            recorded_at: entry.recorded_at,
            seed_prefix: seedPrefix,
            policy_seed: policySeed,
            game_seed: gameSeed,
            reason: stall.reason,
          })}\n`,
        );
        console.log(JSON.stringify(entry));
        return;
      }
      const command = chooseRandomAction(summary, random);
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
      await send(status.control, acquired.owner_token, protocolState, command, {
        source: "random_fidelity_collector",
        policy_seed: policySeed,
        game_seed: gameSeed,
        action_index: actionIndex,
        action_count: enumerateGameplayActions(summary).length,
      });
      pendingEventLeave = eventLeaveMode;
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
      const errorPath = path.join(root, "simulator", "target", "random-fidelity", "errors.log");
      fs.mkdirSync(path.dirname(errorPath), { recursive: true });
      fs.appendFileSync(errorPath, `[${new Date().toISOString()}] ${detail}\n`);
    } catch {}
    process.exit(1);
  });
}

module.exports = {
  addCollectionMetadata,
  chooseRandomAction,
  createCombatStallTracker,
  currentRunRecords,
  enumerateGameplayActions,
  immutableTracePath,
  loadBossUnlocks,
  localBridgeTracePath,
  isSoleEventLeaveScreen,
  communicationBoundary,
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
};
