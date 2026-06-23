#!/usr/bin/env node

const assert = require("assert");
const { checkPreflightFrom } = require("./overnight_preflight");

function freshInput(overrides = {}) {
  return {
    summary: {
      step: 10,
      ready_for_command: true,
      screen_type: "MAP",
      available_commands: ["choose", "state"],
    },
    status: {
      step: 10,
      status: "waiting",
      trace_path: "trace.jsonl",
    },
    summaryAgeMs: 1000,
    statusAgeMs: 900,
    commandExists: false,
    staleThresholdMs: 120000,
    harvestReport: null,
    ...overrides,
  };
}

function testFreshSessionPasses() {
  const result = checkPreflightFrom(freshInput());
  assert.strictEqual(result.ok, true);
  assert.deepStrictEqual(result.problems, []);
  assert.strictEqual(result.summary.screen_type, "MAP");
}

function testStaleSessionFails() {
  const result = checkPreflightFrom(freshInput({ summaryAgeMs: 121000, statusAgeMs: 122000 }));
  assert.strictEqual(result.ok, false);
  assert.match(result.problems.join("\n"), /session files stale/);
}

function testSentCommandAheadOfSummaryFails() {
  const result = checkPreflightFrom(
    freshInput({
      summary: { step: 409, ready_for_command: true, available_commands: ["choose", "state"] },
      status: { step: 410, status: "sent", command: "CHOOSE 0", trace_path: "trace.jsonl" },
    }),
  );
  assert.strictEqual(result.ok, false);
  assert.match(result.problems.join("\n"), /newer than summary/);
}

function testExistingCommandFileFails() {
  const result = checkPreflightFrom(freshInput({ commandExists: true }));
  assert.strictEqual(result.ok, false);
  assert.match(result.problems.join("\n"), /next_command/);
}

function testHarvestReportSummaryIncluded() {
  const result = checkPreflightFrom(
    freshInput({
      harvestReport: {
        updated_at: "2026-06-23T00:00:00.000Z",
        reason: "collector_exit",
        best_run: {
          actions: 323,
          max_floor: 8,
          elite_rooms: 1,
          deaths: 0,
          extracted_path: "best.jsonl",
        },
      },
    }),
  );
  assert.strictEqual(result.harvest_report.best_run.max_floor, 8);
  assert.strictEqual(result.harvest_report.best_run.elite_rooms, 1);
}

testFreshSessionPasses();
testStaleSessionFails();
testSentCommandAheadOfSummaryFails();
testExistingCommandFileFails();
testHarvestReportSummaryIncluded();

console.log("overnight_preflight tests passed");
