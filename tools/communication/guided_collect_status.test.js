#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { inspectGuidedCollectReport, validateTrace } = require("./guided_collect_status");

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function writeJsonl(filePath, records) {
  fs.writeFileSync(filePath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
}

function withTempDir(fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "sts-guided-status-"));
  try {
    fn(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

function testValidateTraceReportsMissingFile() {
  const result = validateTrace("does-not-exist.jsonl");
  assert.strictEqual(result.exists, false);
  assert.strictEqual(result.ok, false);
}

function testInspectMissingReport() {
  const result = inspectGuidedCollectReport("does-not-exist.json");
  assert.strictEqual(result.ok, false);
  assert.match(result.error, /not found/);
}

function testInspectBlockedReport() {
  withTempDir((dir) => {
    const reportPath = path.join(dir, "latest.json");
    writeJson(reportPath, {
      ok: false,
      run_id: 123,
      seed: null,
      stop_reason: "preflight_blocked",
      actions_sent: 0,
      tcp_control_available: false,
      blocker: {
        reason: "bridge_preflight",
        problems: ["session files are stale"],
        warnings: ["TCP bridge control is not available"],
      },
    });

    const result = inspectGuidedCollectReport(reportPath);
    assert.strictEqual(result.ok, false);
    assert.strictEqual(result.run_id, 123);
    assert.strictEqual(result.stop_reason, "preflight_blocked");
    assert.strictEqual(result.blocker.reason, "bridge_preflight");
    assert.deepStrictEqual(result.blocker.problems, ["session files are stale"]);
  });
}

function testInspectReportValidatesTrace() {
  withTempDir((dir) => {
    const tracePath = path.join(dir, "trace.jsonl");
    writeJsonl(tracePath, [
      { type: "action", step: 1, command: "START IRONCLAD 0 GUIDED01" },
      {
        type: "state",
        step: 1,
        message: {
          game_state: {
            floor: 1,
            screen_type: "MAP",
            room_type: "MonsterRoom",
          },
        },
      },
    ]);
    const reportPath = path.join(dir, "latest.json");
    writeJson(reportPath, {
      ok: true,
      run_id: 123,
      seed: "GUIDED01",
      stop_reason: "max_actions",
      actions_sent: 1,
      tcp_control_available: true,
      trace_path: tracePath,
      history_tail: [{ event: "start" }],
    });

    const result = inspectGuidedCollectReport(reportPath);
    assert.strictEqual(result.ok, true);
    assert.strictEqual(result.trace.ok, true);
    assert.strictEqual(result.trace.actions, 1);
    assert.strictEqual(result.trace.max_floor, 1);
    assert.strictEqual(result.history_tail_count, 1);
  });
}

testValidateTraceReportsMissingFile();
testInspectMissingReport();
testInspectBlockedReport();
testInspectReportValidatesTrace();

console.log("guided_collect_status tests passed");
