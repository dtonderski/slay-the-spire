#!/usr/bin/env node

const crypto = require("crypto");
const fs = require("fs");
const net = require("net");
const path = require("path");
const readline = require("readline");
const { spawnSync } = require("child_process");

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

function enumerateGameplayActions(summary) {
  const available = availableSet(summary);
  const actions = [];
  const choices = Array.isArray(summary?.choices) ? summary.choices : [];

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
        normalizedChoice.includes("potion")
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

function semanticStateKey(protocolState) {
  const state = protocolState?.state?.message ?? protocolState?.summary;
  if (!state) return null;
  const scrub = (value) => {
    if (Array.isArray(value)) return value.map(scrub);
    if (!value || typeof value !== "object") return value;
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => !["step", "state_id", "state_seq", "playtime_seconds"].includes(key))
        .map(([key, nested]) => [key, scrub(nested)]),
    );
  };
  return JSON.stringify(scrub(state));
}

function stateAdvanced(previousStateKey, protocolState) {
  const next = semanticStateKey(protocolState);
  return Boolean(previousStateKey && next && next !== previousStateKey);
}

function needsGameplaySettlePoll(previousStateKey, protocolState) {
  return (
    !protocolState?.summary?.ready_for_command || !stateAdvanced(previousStateKey, protocolState)
  );
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

function verificationCheckpointKey(summary) {
  if (!summary) return null;
  return JSON.stringify({
    in_game: Boolean(summary.in_game),
    act: summary.act ?? null,
    floor: summary.floor ?? null,
    room_type: summary.room_type ?? null,
  });
}

function shouldVerifyTrace({
  actionCount,
  lastVerifiedActionCount,
  checkpointKey,
  lastVerifiedCheckpointKey,
  interval,
  terminal,
}) {
  return (
    lastVerifiedActionCount === null ||
    terminal ||
    checkpointKey !== lastVerifiedCheckpointKey ||
    actionCount - lastVerifiedActionCount >= interval
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
    time_eater_seen: seen("TIME", "TIME EATER", "TIME_EATER"),
  };
}

function loadBossUnlocks(environment = process.env) {
  if (environment.STS_BOSS_UNLOCKS_JSON) {
    return parseBossUnlocks(environment.STS_BOSS_UNLOCKS_JSON);
  }
  if (!environment.STS_SEEN_BOSSES_PATH) return parseBossUnlocks();
  const preferencesPath = path.resolve(environment.STS_SEEN_BOSSES_PATH);
  return parseSeenBossesPreferences(fs.readFileSync(preferencesPath, "utf8"));
}

function addCollectionMetadata(
  records,
  bossUnlocks,
  policySeed,
  gameSeed,
  startingHp,
  sourceVersion = "working-tree",
) {
  const metadata = {
    type: "metadata",
    schema: 1,
    source: "communication_mod",
    client: "tools/communication/random_fidelity_collector.js",
    source_version: sourceVersion,
    boss_unlocks: bossUnlocks,
    collection: { policy_seed: policySeed, game_seed: gameSeed, starting_hp: startingHp },
  };
  return [metadata, ...records.filter((record) => record.type !== "metadata")];
}

function normalizeSettledGameplayRecords(records) {
  records = records.filter((record) =>
    ["metadata", "action", "state", "error"].includes(record.type),
  );
  const normalized = [];
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index];
    if (record.type !== "action" || String(record.command).toUpperCase() === "STATE") {
      normalized.push(record);
      continue;
    }
    const immediate = records[index + 1];
    if (!immediate || immediate.type !== "state") {
      normalized.push(record);
      continue;
    }
    let settled = immediate;
    let cursor = index + 2;
    while (cursor + 1 < records.length) {
      const poll = records[cursor];
      const pollState = records[cursor + 1];
      if (
        poll.type !== "action" ||
        poll.command_meta?.metadata?.operator_control !== "settle_gameplay" ||
        poll.command_meta?.metadata?.reason === "confirm_event_leave" ||
        pollState.type !== "state"
      ) {
        break;
      }
      settled = pollState;
      cursor += 2;
    }
    normalized.push(record, { ...settled, step: record.step });
    index = cursor - 1;
  }
  return normalized;
}

function promoteDistinctFailure({ minimizedPath, fingerprint: id, boundaryPath, boundaryCategory }) {
  if (!minimizedPath || !fs.existsSync(minimizedPath)) return null;
  const corpusDir = path.join(root, "simulator", "verification", "corpus", "permanent_traces");
  const manifestPath = path.join(root, "simulator", "verification", "corpus", "permanent_traces.json");
  const lockPath = path.join(root, "simulator", "verification", "corpus", ".permanent-traces.lock");
  const traceName = `random-fidelity-${id}.jsonl`;
  const destination = path.join(corpusDir, traceName);
  acquireDirectoryLock(lockPath);
  try {
    const manifestText = fs.readFileSync(manifestPath, "utf8");
    const manifest = JSON.parse(manifestText);
    if (manifest.entries.some((entry) => entry.trace === traceName)) {
      return { trace: destination, added: false };
    }
    fs.copyFileSync(minimizedPath, destination, fs.constants.COPYFILE_EXCL);
    const entry = {
      trace: traceName,
      expectation: {
        kind: "expected_boundary",
        boundary: { path: boundaryPath, category: boundaryCategory },
      },
    };
    const formattedEntry = JSON.stringify(entry, null, 2)
      .split("\n")
      .map((line) => `    ${line}`)
      .join("\n");
    const updatedManifest = manifestText.replace(/\n  \]\s*\}\s*$/, `,\n${formattedEntry}\n  ]\n}\n`);
    if (updatedManifest === manifestText) throw new Error("permanent trace manifest terminator not found");
    const temporaryManifest = `${manifestPath}.tmp-${process.pid}`;
    fs.writeFileSync(temporaryManifest, updatedManifest);
    fs.renameSync(temporaryManifest, manifestPath);
    return { trace: destination, added: true };
  } catch (error) {
    if (fs.existsSync(destination)) {
      const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
      if (!manifest.entries.some((entry) => entry.trace === traceName)) {
        fs.unlinkSync(destination);
      }
    }
    throw error;
  } finally {
    fs.rmdirSync(lockPath);
  }
}

function acquireDirectoryLock(lockPath, timeoutMs = 30_000, staleAfterMs = 60_000) {
  const lockStarted = Date.now();
  for (;;) {
    try {
      fs.mkdirSync(lockPath);
      return;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
      try {
        const ageMs = Date.now() - fs.statSync(lockPath).mtimeMs;
        if (ageMs >= staleAfterMs) {
          fs.rmdirSync(lockPath);
          continue;
        }
      } catch (recoveryError) {
        if (recoveryError.code === "ENOENT") continue;
        if (recoveryError.code !== "ENOTEMPTY") throw recoveryError;
      }
      if (Date.now() - lockStarted >= timeoutMs) {
        throw new Error(`timed out locking permanent trace manifest: ${lockPath}`);
      }
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 25);
    }
  }
}

function parseParityOutput(output) {
  const value = (name) => {
    const match = output.match(new RegExp(`^${name}=(.*)$`, "m"));
    return match ? match[1].trim() : null;
  };
  const firstDiff = output.match(/^unexpected_diff step=(\d+) command="([^"]*)" label="([^"]*)"$/m);
  const diffLines = [];
  const lines = output.split(/\r?\n/);
  const firstDiffIndex = lines.findIndex((line) => line.startsWith("unexpected_diff "));
  if (firstDiffIndex >= 0) {
    for (let index = firstDiffIndex + 1; index < lines.length && /^\s{2}/.test(lines[index]); index += 1) {
      diffLines.push(lines[index].trim());
    }
  }
  return {
    outcome: value("outcome"),
    unexpectedDiffs: Number.parseInt(value("unexpected_diffs") || "0", 10),
    duplicateDispositions: Number.parseInt(value("duplicate_dispositions") || "0", 10),
    unresolvedTransientAssertions: Number.parseInt(
      value("unresolved_transient_assertions") || "0",
      10,
    ),
    terminalStateObserved: value("terminal_state_observed") === "true",
    boundaryPath: value("seed_start.first_boundary.path"),
    boundaryCategory: value("seed_start.first_boundary.category"),
    boundaryReason: value("seed_start.first_boundary.reason"),
    firstDiff: firstDiff
      ? { step: Number(firstDiff[1]), command: firstDiff[2], label: firstDiff[3] }
      : null,
    diffLines,
  };
}

function fingerprint(result) {
  const source = result.firstDiff
    ? `diff|${result.firstDiff.command}|${result.firstDiff.label}|${(result.diffLines || []).join("|")}`
    : `boundary|${result.boundaryCategory}|${String(result.boundaryPath || "").replace(/step=\d+/g, "step=*")}|${result.boundaryReason || ""}`;
  return crypto.createHash("sha256").update(source).digest("hex").slice(0, 16);
}

function expectedFailureBoundary(result) {
  const boundaryStep = Number.parseInt(
    String(result.boundaryPath || "").match(/step=(\d+)/)?.[1] || "",
    10,
  );
  if (result.firstDiff && (!Number.isInteger(boundaryStep) || result.firstDiff.step < boundaryStep)) {
    return {
      path: `$.actions[step=${result.firstDiff.step}].command`,
      category: "unexpected_sim_real_diff",
    };
  }
  return { path: result.boundaryPath, category: result.boundaryCategory };
}

function isPromotableFailure(result) {
  if (String(result.boundaryCategory).toLowerCase() === "unsupported_neow_boss_swap") {
    // The first visible relic mismatch is the verifier's explanation for the
    // unsupported Neow boss-swap RNG path, not an independently replayable
    // simulator mechanic. Preserve the trace, but do not promote it.
    return false;
  }
  return result.unexpectedDiffs > 0 &&
    Number(result.duplicateDispositions || 0) === 0 &&
    expectedFailureBoundary(result).category === "unexpected_sim_real_diff";
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

function verifierInvocationFailed(child) {
  // On WSL/DrvFS, Node can report a cleanup-time EPERM even though the child
  // ran to completion and returned a valid status plus output. Trust the
  // completed status; a genuine spawn failure has a null status.
  return ![0, 2].includes(child.status);
}

function verifyTrace(verifierPath, tracePath) {
  const child = spawnSync(verifierPath, ["parity", tracePath], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  const output = `${child.stdout || ""}\n${child.stderr || ""}`;
  if (verifierInvocationFailed(child)) {
    throw new Error(
      `verifier invocation failed with exit ${child.status}: ${child.error?.message || output.trim()}`,
    );
  }
  return { ...parseParityOutput(output), exitCode: child.status, output };
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
  if (
    response.observed_update &&
    !response.observed_update.pending_command &&
    Number(response.observed_update.state_seq) > Number(protocolState.state_seq)
  ) {
    return response.observed_update;
  }

  // A long animation can outlive update_timeout_ms even though the bridge has
  // accepted and queued the command. Do not submit a settle STATE into that
  // occupied slot; wait for the accepted command to finish instead.
  for (let attempt = 0; attempt < 240; attempt += 1) {
    const current = await controlRequest(control, { type: "state" });
    if (
      current.ok &&
      !current.pending_command &&
      Number(current.state_seq) > Number(protocolState.state_seq)
    ) {
      return current;
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
  const verifierPath = path.resolve(
    process.env.STS_VERIFY_BIN || path.join(root, "simulator", "target", "debug", "sts_verify.exe"),
  );
  if (!fs.existsSync(verifierPath)) throw new Error(`verifier binary not found: ${verifierPath}`);

  const policySeed = Number.parseInt(process.env.STS_RANDOM_POLICY_SEED || "1", 10);
  const gameSeed = process.env.STS_GAME_SEED || `COLLECT${policySeed}`;
  const startingHp = Number.parseInt(process.env.STS_STARTING_HP || "10000", 10);
  const maxActions = Number.parseInt(process.env.STS_RANDOM_MAX_ACTIONS || "10000", 10);
  const logActions = process.env.STS_RANDOM_LOG_ACTIONS !== "0";
  const verifyEvery = Number.parseInt(process.env.STS_RANDOM_VERIFY_EVERY || "50", 10);
  const deferVerification = process.env.STS_RANDOM_DEFER_VERIFICATION === "1";
  const sourceVersion = process.env.STS_RANDOM_SOURCE_VERSION || "working-tree";
  const traceStartedAt = new Date();
  const gameplaySettleTimeoutMs = Number.parseInt(
    process.env.STS_RANDOM_SETTLE_TIMEOUT_MS || "20000",
    10,
  );
  if (!Number.isInteger(verifyEvery) || verifyEvery < 1) {
    throw new Error("STS_RANDOM_VERIFY_EVERY must be a positive integer");
  }
  if (!Number.isInteger(gameplaySettleTimeoutMs) || gameplaySettleTimeoutMs < 1) {
    throw new Error("STS_RANDOM_SETTLE_TIMEOUT_MS must be a positive integer");
  }
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
  const activeTrace = deferVerification
    ? immutableTracePath(campaignDir, gameSeed, policySeed, traceStartedAt)
    : path.join(campaignDir, `active-${gameSeed}-p${policySeed}.jsonl`);
  const ledgerPath = path.join(campaignDir, "ledger.jsonl");

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
    if (!startupReady) {
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
    }
    if (!startupReady &&
        !availableSet(protocolState.summary).has("start") &&
        !availableSet(protocolState.summary).has("start_verify")) {
      throw new Error("START_VERIFY did not become available at the main menu");
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
    let pendingGameplaySummaryKey = null;
    let pendingEventLeave = null;
    let gameplaySettleStartedAt = null;
    let lastVerifiedActionCount = null;
    let lastVerifiedCheckpointKey = null;
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
      if (
        pendingGameplaySummaryKey &&
        needsGameplaySettlePoll(pendingGameplaySummaryKey, protocolState)
      ) {
        if (
          gameplaySettleStartedAt !== null
          && Date.now() - gameplaySettleStartedAt >= gameplaySettleTimeoutMs
        ) {
          throw new Error("game state did not advance after accepted gameplay command");
        }
        await send(status.control, acquired.owner_token, protocolState, "STATE", {
          source: "random_fidelity_collector",
          policy_seed: policySeed,
          game_seed: gameSeed,
          action_index: actionIndex,
          operator_control: "settle_gameplay",
        });
        actionIndex -= 1;
        continue;
      }
      if (pendingGameplaySummaryKey) {
        pendingGameplaySummaryKey = null;
        gameplaySettleStartedAt = null;
      }
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
      const checkpointKey = verificationCheckpointKey(summary);
      const terminal = !summary.in_game || actionIndex > maxActions;
      const verificationDue = deferVerification
        ? terminal
        : shouldVerifyTrace({
            actionCount,
            lastVerifiedActionCount,
            checkpointKey,
            lastVerifiedCheckpointKey,
            interval: verifyEvery,
            terminal,
          });
      if (verificationDue) {
        const rawRecords = addCollectionMetadata(
          await currentRunRecords(bridgeTracePath, runTraceStartOffset),
          bossUnlocks,
          policySeed,
          gameSeed,
          startingHp,
          sourceVersion,
        );
        const records = normalizeSettledGameplayRecords(rawRecords);
        writeTrace(activeTrace, records, { exclusive: deferVerification });
        if (deferVerification) {
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
          fs.appendFileSync(
            path.join(campaignDir, "verification_queue.jsonl"),
            `${JSON.stringify(entry)}\n`,
          );
          console.log(JSON.stringify(entry));
          return;
        }
        // The bridge keeps recording every settled action between checks. A
        // failed batch is minimized below, recovering its first divergent action
        // even though the real game may have advanced farther.
        const verification = verifyTrace(verifierPath, activeTrace);
        lastVerifiedActionCount = actionCount;
        lastVerifiedCheckpointKey = checkpointKey;
        const terminalBoundary =
          verification.unexpectedDiffs > 0 ||
          (verification.boundaryCategory && verification.boundaryCategory !== "none");
        if (terminalBoundary) {
          const id = fingerprint(verification);
          const archivePath = path.join(campaignDir, "raw", `${gameSeed}-p${policySeed}-${id}.jsonl`);
          const minimizedPath = path.join(
            campaignDir,
            "minimized",
            `${gameSeed}-p${policySeed}-${id}.jsonl`,
          );
          writeTrace(archivePath, rawRecords);
          fs.mkdirSync(path.dirname(minimizedPath), { recursive: true });
          const minimized = spawnSync(
            verifierPath,
            ["minimize", "-o", minimizedPath, activeTrace],
            { cwd: root, encoding: "utf8", windowsHide: true },
          );
          if (minimized.status !== 0) {
            throw new Error(`trace minimization failed: ${minimized.stderr || minimized.stdout}`);
          }
          const expectedBoundary = expectedFailureBoundary(verification);
          const promotion =
            isPromotableFailure(verification)
              ? promoteDistinctFailure({
                  minimizedPath,
                  fingerprint: id,
                  boundaryPath: expectedBoundary.path,
                  boundaryCategory: expectedBoundary.category,
                })
              : null;
          const entry = {
            recorded_at: new Date().toISOString(),
            game_seed: gameSeed,
            policy_seed: policySeed,
            starting_hp: startingHp,
            actions: actionIndex - 1,
            fingerprint: id,
            kind: verification.unexpectedDiffs > 0 ? "divergence" : "boundary",
            boundary_category: verification.boundaryCategory,
            boundary_path: verification.boundaryPath,
            boundary_reason: verification.boundaryReason,
            first_diff: verification.firstDiff,
            diff_lines: verification.diffLines,
            raw_trace: archivePath,
            minimized_trace: fs.existsSync(minimizedPath) ? minimizedPath : null,
            permanent_trace: promotion?.trace || null,
            newly_promoted: promotion?.added || false,
          };
          fs.appendFileSync(ledgerPath, `${JSON.stringify(entry)}\n`);
          console.log(JSON.stringify(entry, null, 2));
          process.exitCode = 2;
          return;
        }
      }
      if (!summary.in_game) {
        const entry = {
          recorded_at: new Date().toISOString(),
          kind: "complete",
          game_seed: gameSeed,
          policy_seed: policySeed,
          starting_hp: startingHp,
          actions: actionIndex - 1,
          active_trace: activeTrace,
        };
        fs.appendFileSync(ledgerPath, `${JSON.stringify(entry)}\n`);
        console.log(JSON.stringify(entry));
        return;
      }
      if (actionIndex > maxActions) {
        const entry = {
          recorded_at: new Date().toISOString(),
          kind: "max_actions",
          game_seed: gameSeed,
          policy_seed: policySeed,
          starting_hp: startingHp,
          actions: maxActions,
          active_trace: activeTrace,
        };
        fs.appendFileSync(ledgerPath, `${JSON.stringify(entry)}\n`);
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
      const sourceSummaryKey = semanticStateKey(protocolState);
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
      pendingGameplaySummaryKey = sourceSummaryKey;
      gameplaySettleStartedAt = Date.now();
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
  acquireDirectoryLock,
  addCollectionMetadata,
  chooseRandomAction,
  currentRunRecords,
  enumerateGameplayActions,
  expectedFailureBoundary,
  fingerprint,
  immutableTracePath,
  loadBossUnlocks,
  localBridgeTracePath,
  isPromotableFailure,
  isSoleEventLeaveScreen,
  needsGameplaySettlePoll,
  needsMapChoiceSettle,
  normalizeSettledGameplayRecords,
  parseParityOutput,
  parseBossUnlocks,
  parseSeenBossesPreferences,
  promoteDistinctFailure,
  seededRandom,
  semanticStateKey,
  shouldVerifyTrace,
  stateAdvanced,
  verificationCheckpointKey,
  verifierInvocationFailed,
  verifyTrace,
  writeTrace,
};
