#!/usr/bin/env node

const assert = require("assert");
const {
  bridgeLooksStaleFrom,
  currentTracePathFromStatus,
} = require("./overnight_supervisor");

function testNoSessionFilesAreStale() {
  const result = bridgeLooksStaleFrom({
    summary: null,
    status: null,
    summaryAgeMs: Number.POSITIVE_INFINITY,
    statusAgeMs: Number.POSITIVE_INFINITY,
    staleThresholdMs: 120000,
  });
  assert.strictEqual(result.stale, true);
  assert.match(result.reason, /no session/);
}

function testOldSessionFilesAreStale() {
  const result = bridgeLooksStaleFrom({
    summary: { status: "waiting" },
    status: { status: "waiting" },
    summaryAgeMs: 121000,
    statusAgeMs: 122000,
    staleThresholdMs: 120000,
  });
  assert.strictEqual(result.stale, true);
  assert.match(result.reason, /session files stale/);
}

function testExitedBridgeIsStale() {
  const result = bridgeLooksStaleFrom({
    summary: { status: "waiting" },
    status: { status: "exited", reason: "child process ended" },
    summaryAgeMs: 10,
    statusAgeMs: 10,
    staleThresholdMs: 120000,
  });
  assert.strictEqual(result.stale, true);
  assert.match(result.reason, /bridge exited/);
}

function testFreshSessionIsActive() {
  const result = bridgeLooksStaleFrom({
    summary: { status: "waiting" },
    status: { status: "waiting", trace_path: "trace.jsonl" },
    summaryAgeMs: 1000,
    statusAgeMs: 900,
    staleThresholdMs: 120000,
  });
  assert.deepStrictEqual(result, { stale: false, reason: "session active" });
}

function testTracePathExtraction() {
  assert.strictEqual(currentTracePathFromStatus({ trace_path: "abc.jsonl" }), "abc.jsonl");
  assert.strictEqual(currentTracePathFromStatus({}), null);
  assert.strictEqual(currentTracePathFromStatus(null), null);
}

testNoSessionFilesAreStale();
testOldSessionFilesAreStale();
testExitedBridgeIsStale();
testFreshSessionIsActive();
testTracePathExtraction();

console.log("overnight_supervisor tests passed");
