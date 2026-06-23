#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

function usage() {
  console.error("Usage:");
  console.error("  node tools/communication/trace_tools.js validate <trace.jsonl>");
  console.error("  node tools/communication/trace_tools.js trim-valid-prefix <input.jsonl> <output.jsonl>");
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
  for (const record of records) {
    const gs = record.message?.game_state;
    if (!gs) continue;
    if (gs.floor != null) floors.add(gs.floor);
    const monsters = gs.combat_state?.monsters?.map((monster) => monster.id || monster.name).join("+");
    if (monsters) encounters.add(monsters);
  }
  return {
    records: records.length,
    states,
    errors,
    actions,
    max_floor: floors.size ? Math.max(...floors) : null,
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

usage();
