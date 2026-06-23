#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

function usage() {
  console.error("Usage:");
  console.error("  node tools/communication/trace_tools.js validate <trace.jsonl>");
  console.error("  node tools/communication/trace_tools.js trim-valid-prefix <input.jsonl> <output.jsonl>");
  console.error("  node tools/communication/trace_tools.js extract-run <input.jsonl> <run-index> <output.jsonl>");
  console.error("  node tools/communication/trace_tools.js collapse-card-reward-loop <input.jsonl> <output.jsonl>");
  process.exit(2);
}

function readTrace(filePath) {
  return fs
    .readFileSync(filePath, "utf8")
    .split(/\r?\n/)
    .filter((line) => line.trim().length > 0)
    .map((line, index) => {
      try {
        return JSON.parse(line);
      } catch (error) {
        throw new Error(`${filePath}:${index + 1}: ${error.message}`);
      }
    });
}

function writeTrace(filePath, records) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
}

function matchingResponseSteps(records) {
  const responseSteps = new Set();
  for (const record of records) {
    if (record.type === "state" || record.type === "error") {
      responseSteps.add(record.step);
    }
  }
  return responseSteps;
}

function missingActionResponses(records) {
  const responseSteps = matchingResponseSteps(records);
  return records
    .map((record, index) => ({ record, index }))
    .filter(({ record }) => record.type === "action" && !responseSteps.has(record.step));
}

function summarize(records) {
  const states = records.filter((record) => record.type === "state").length;
  const errors = records.filter((record) => record.type === "error").length;
  const actions = records.filter((record) => record.type === "action").length;
  const floors = new Set();
  const encounters = new Set();
  const seeds = new Set();
  const starts = [];
  const rooms = [];
  const bosses = new Set();
  let deaths = 0;
  let lastRoomKey = "";
  for (const record of records) {
    if (record.type === "action" && /^START\s+/i.test(record.command || "")) {
      starts.push({ step: record.step, command: record.command });
    }
    const gs = record.message?.game_state;
    if (!gs) continue;
    if (gs.floor != null) floors.add(gs.floor);
    if (gs.seed != null) seeds.add(gs.seed);
    if (gs.act_boss) bosses.add(gs.act_boss);
    if (gs.screen_type === "GAME_OVER" || gs.current_hp === 0) deaths += 1;
    if (gs.floor != null && gs.room_type) {
      const roomKey = `${gs.floor}:${gs.room_type}:${gs.room_phase || ""}`;
      if (roomKey !== lastRoomKey) {
        rooms.push({
          floor: gs.floor,
          room_type: gs.room_type,
          room_phase: gs.room_phase || null,
          screen_type: gs.screen_type || null,
        });
        lastRoomKey = roomKey;
      }
    }
    const monsters = gs.combat_state?.monsters?.map((monster) => monster.id || monster.name).join("+");
    if (monsters) encounters.add(monsters);
  }
  return {
    records: records.length,
    states,
    errors,
    actions,
    starts,
    seeds: [...seeds],
    act_bosses: [...bosses],
    deaths,
    max_floor: floors.size ? Math.max(...floors) : null,
    rooms,
    encounters: [...encounters],
  };
}

function validate(records) {
  const missing = missingActionResponses(records).map(({ record, index }) => ({
    line: index + 1,
    step: record.step,
    command: record.command,
  }));
  return { ok: missing.length === 0, missing, summary: summarize(records) };
}

function trimValidPrefix(records, sourcePath) {
  const missing = missingActionResponses(records);
  if (missing.length === 0) return { records, trimmed: false, missing: [] };

  const firstMissing = missing[0];
  const kept = records.slice(0, firstMissing.index);
  kept.push({
    type: "metadata",
    event: "truncated",
    reason: "missing_state_after_action",
    source_trace: path.basename(sourcePath),
    missing_step: firstMissing.record.step,
    missing_command: firstMissing.record.command,
    truncated_at_line: firstMissing.index + 1,
    created_at: new Date().toISOString(),
  });
  return {
    records: kept,
    trimmed: true,
    missing: missing.map(({ record, index }) => ({
      line: index + 1,
      step: record.step,
      command: record.command,
    })),
  };
}

function extractRun(records, sourcePath, runIndex) {
  const starts = records
    .map((record, index) => ({ record, index }))
    .filter(({ record }) => record.type === "action" && /^START\s+/i.test(record.command || ""));
  const selected = starts[runIndex];
  if (!selected) {
    throw new Error(`run index ${runIndex} not found; found ${starts.length} START actions`);
  }
  const next = starts[runIndex + 1];
  const firstIndex =
    selected.index > 0 && records[selected.index - 1].type === "state"
      ? selected.index - 1
      : selected.index;
  const lastIndex = next ? next.index - 1 : records.length;
  const stepOffset = selected.record.step - 1;
  const extracted = records.slice(firstIndex, lastIndex).map((record) => {
    const copy = { ...record };
    if (typeof copy.step === "number") copy.step -= stepOffset;
    return copy;
  });
  extracted.unshift({
    type: "metadata",
    schema: 1,
    source: "communication_mod",
    event: "extracted_run",
    source_trace: path.basename(sourcePath),
    source_run_index: runIndex,
    source_start_step: selected.record.step,
    created_at: new Date().toISOString(),
  });
  return extracted;
}

function cardRewardSignature(record) {
  const game = record?.message?.game_state;
  if (record?.type !== "state" || game?.screen_type !== "CARD_REWARD") return null;
  const choices = (game.choice_list || []).map((choice) =>
    typeof choice === "string" ? choice : choice?.label || ""
  );
  const cards = (game.screen_state?.cards || []).map((card) => `${card.id || ""}:${card.name || ""}`);
  return JSON.stringify({ choices, cards, floor: game.floor, gold: game.gold, deck: (game.deck || []).length });
}

function isPendingCombatCardReward(record) {
  const game = record?.message?.game_state;
  if (record?.type !== "state" || game?.screen_type !== "COMBAT_REWARD") return false;
  return (game.screen_state?.rewards || []).some((reward) => reward.reward_type === "CARD");
}

function collapseCardRewardLoop(records, sourcePath) {
  const collapsed = [];
  let removedPairs = 0;
  for (let index = 0; index < records.length; index += 1) {
    const previousSignature = cardRewardSignature(collapsed[collapsed.length - 1]);
    const skipAction = records[index];
    const skipState = records[index + 1];
    const reopenAction = records[index + 2];
    const reopenState = records[index + 3];
    if (
      previousSignature &&
      skipAction?.type === "action" &&
      skipAction.command?.trim().toUpperCase() === "SKIP" &&
      isPendingCombatCardReward(skipState) &&
      reopenAction?.type === "action" &&
      /^CHOOSE\s+\d+$/i.test(reopenAction.command || "") &&
      cardRewardSignature(reopenState) === previousSignature
    ) {
      removedPairs += 1;
      index += 3;
      continue;
    }
    collapsed.push(records[index]);
  }
  if (removedPairs > 0) {
    collapsed.unshift({
      type: "metadata",
      schema: 1,
      source: "communication_mod",
      event: "collapsed_card_reward_loop",
      source_trace: path.basename(sourcePath),
      removed_skip_reopen_pairs: removedPairs,
      created_at: new Date().toISOString(),
    });
  }
  return { records: collapsed, removedPairs };
}

const [command, inputPath, outputPath] = process.argv.slice(2);
if (!command || !inputPath) usage();

if (command === "validate") {
  const result = validate(readTrace(inputPath));
  console.log(JSON.stringify(result, null, 2));
  process.exit(result.ok ? 0 : 1);
}

if (command === "trim-valid-prefix") {
  if (!outputPath) usage();
  const result = trimValidPrefix(readTrace(inputPath), inputPath);
  writeTrace(outputPath, result.records);
  const validation = validate(result.records);
  console.log(JSON.stringify({ ...result, records: result.records.length, validation }, null, 2));
  process.exit(validation.ok ? 0 : 1);
}

if (command === "extract-run") {
  const runIndex = Number.parseInt(outputPath, 10);
  const destination = process.argv[5];
  if (!Number.isInteger(runIndex) || !destination) usage();
  const extracted = extractRun(readTrace(inputPath), inputPath, runIndex);
  writeTrace(destination, extracted);
  const validation = validate(extracted);
  console.log(JSON.stringify({ records: extracted.length, validation }, null, 2));
  process.exit(validation.ok ? 0 : 1);
}

if (command === "collapse-card-reward-loop") {
  if (!outputPath) usage();
  const result = collapseCardRewardLoop(readTrace(inputPath), inputPath);
  writeTrace(outputPath, result.records);
  const validation = validate(result.records);
  console.log(JSON.stringify({ ...result, records: result.records.length, validation }, null, 2));
  process.exit(validation.ok ? 0 : 1);
}

usage();
