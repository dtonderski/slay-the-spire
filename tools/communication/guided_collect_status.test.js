#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { inspectGuidedCollectReport, recentReports, validateTrace } = require("./guided_collect_status");

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
      producer: "sts.guided_collect",
      generated_at: "2026-06-30T12:00:00Z",
      run_id: 123,
      seed: null,
      stop_reason: "preflight_blocked",
      actions_sent: 0,
      tcp_control_available: false,
      selection: {
        mode: "auto",
        selected_run_id: 456,
        considered_count: 2,
        candidate_count: 25,
        skipped_unsupported: [{ run_id: 123, reason: "unsupported_neow_followup" }],
      },
      preflight: {
        ok: false,
        ages: { status_age_seconds: 1, summary_age_seconds: 2 },
        pending_command: { present: false, transport: null },
        summary: { step: 7 },
        status: { status: "waiting" },
      },
      blocker: {
        reason: "bridge_preflight",
        problems: ["session files are stale"],
        warnings: ["TCP bridge control is not available"],
      },
      trace_validation: {
        verified: false,
        stop_reason: "observed_state_diff",
        steps: 4,
        blocker: {
          reason: "observed_state_diff",
          detail: "hp differs",
        },
      },
    });

    const result = inspectGuidedCollectReport(reportPath);
    assert.strictEqual(result.ok, false);
    assert.strictEqual(result.producer, "sts.guided_collect");
    assert.strictEqual(result.generated_at, "2026-06-30T12:00:00Z");
    assert.match(result.report_modified_at, /^\d{4}-\d{2}-\d{2}T/);
    assert.strictEqual(typeof result.report_age_seconds, "number");
    assert.strictEqual(result.run_id, 123);
    assert.strictEqual(result.stop_reason, "preflight_blocked");
    assert.strictEqual(result.selection.mode, "auto");
    assert.strictEqual(result.selection.selected_run_id, 456);
    assert.strictEqual(result.selection.considered_count, 2);
    assert.strictEqual(result.selection.candidate_count, 25);
    assert.strictEqual(result.selection.skipped_unsupported_count, 1);
    assert.strictEqual(result.preflight.ok, false);
    assert.strictEqual(result.preflight.ages.summary_age_seconds, 2);
    assert.strictEqual(result.preflight.pending_command.present, false);
    assert.strictEqual(result.preflight.summary.step, 7);
    assert.strictEqual(result.blocker.reason, "bridge_preflight");
    assert.deepStrictEqual(result.blocker.problems, ["session files are stale"]);
    assert.strictEqual(result.strict_trace_validation.verified, false);
    assert.strictEqual(result.strict_trace_validation.stop_reason, "observed_state_diff");
    assert.strictEqual(result.strict_trace_validation.blocker_reason, "observed_state_diff");
    assert.strictEqual(result.strict_trace_validation.blocker_detail, "hp differs");
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
      producer: "sts.guided_collect",
      generated_at: "2026-06-30T12:00:00Z",
      run_id: 123,
      seed: "GUIDED01",
      stop_reason: "max_actions",
      actions_sent: 1,
      tcp_control_available: true,
      trace_path: tracePath,
      trace_validation: {
        verified: true,
        stop_reason: "trace_exhausted",
        steps: 1,
        final_phase: "map",
      },
      history_tail: [{ event: "start" }],
    });

    const result = inspectGuidedCollectReport(reportPath);
    assert.strictEqual(result.ok, true);
    assert.strictEqual(result.producer, "sts.guided_collect");
    assert.strictEqual(result.trace.ok, true);
    assert.strictEqual(result.trace.actions, 1);
    assert.strictEqual(result.trace.max_floor, 1);
    assert.strictEqual(result.strict_trace_validation.verified, true);
    assert.strictEqual(result.strict_trace_validation.steps, 1);
    assert.strictEqual(result.history_tail_count, 1);
  });
}

function testRecentReportsSortsNewestFirst() {
  withTempDir((dir) => {
    const oldPath = path.join(dir, "old.json");
    const newPath = path.join(dir, "new.json");
    writeJson(oldPath, { ok: false });
    writeJson(newPath, { ok: true });
    const oldTime = new Date("2026-01-01T00:00:00Z");
    const newTime = new Date("2026-01-02T00:00:00Z");
    fs.utimesSync(oldPath, oldTime, oldTime);
    fs.utimesSync(newPath, newTime, newTime);

    const result = recentReports(dir);

    assert.strictEqual(result.length, 2);
    assert.strictEqual(result[0].name, "new.json");
    assert.strictEqual(result[1].name, "old.json");
  });
}

function testInspectReportIncludesRecentReports() {
  withTempDir((dir) => {
    const archiveDir = path.join(dir, "reports");
    fs.mkdirSync(archiveDir);
    writeJson(path.join(archiveDir, "attempt.json"), { ok: false });
    const reportPath = path.join(dir, "latest.json");
    writeJson(reportPath, {
      ok: false,
      stop_reason: "preflight_blocked",
      actions_sent: 0,
    });

    const result = inspectGuidedCollectReport(reportPath, archiveDir);

    assert.strictEqual(result.recent_reports.length, 1);
    assert.strictEqual(result.recent_reports[0].name, "attempt.json");
  });
}

testValidateTraceReportsMissingFile();
testInspectMissingReport();
testInspectBlockedReport();
testInspectReportValidatesTrace();
testRecentReportsSortsNewestFirst();
testInspectReportIncludesRecentReports();

console.log("guided_collect_status tests passed");
