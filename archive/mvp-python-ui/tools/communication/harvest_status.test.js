#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { inspectHarvestReport, validatePath } = require("./harvest_status");

function writeJsonl(filePath, records) {
  fs.writeFileSync(filePath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
}

function withTempDir(fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "sts-harvest-status-"));
  try {
    fn(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

function testValidatePathReportsMissingFile() {
  const result = validatePath("does-not-exist.jsonl");
  assert.strictEqual(result.exists, false);
  assert.strictEqual(result.ok, false);
}

function testInspectHarvestReportValidatesBestRun() {
  withTempDir((dir) => {
    const bestRunPath = path.join(dir, "best.jsonl");
    writeJsonl(bestRunPath, [
      { type: "action", step: 1, command: "START IRONCLAD 0 M290001" },
      {
        type: "state",
        step: 1,
        message: {
          game_state: {
            floor: 7,
            screen_type: "COMBAT_REWARD",
            room_type: "MonsterRoomElite",
            room_phase: "COMPLETE",
            screen_state: { rewards: [{ reward_type: "RELIC" }] },
          },
        },
      },
    ]);
    const reportPath = path.join(dir, "harvest_report.json");
    fs.writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          updated_at: "2026-06-23T00:00:00.000Z",
          reason: "collector_exit",
          trace_path: null,
          validation: { ok: false },
          valid_prefix_path: null,
          best_run: {
            run_index: 1,
            start_step: 87,
            command: "START IRONCLAD 0 M290001",
            extracted_path: bestRunPath,
            encounters: ["Sentry+Sentry+Sentry"],
          },
        },
        null,
        2,
      )}\n`,
    );
    const result = inspectHarvestReport(reportPath);
    assert.strictEqual(result.ok, true);
    assert.strictEqual(result.best_run.ok, true);
    assert.strictEqual(result.best_run.elite_rooms, 1);
    assert.deepStrictEqual(result.best_run.encounters, ["Sentry+Sentry+Sentry"]);
  });
}

testValidatePathReportsMissingFile();
testInspectHarvestReportValidatesBestRun();

console.log("harvest_status tests passed");
