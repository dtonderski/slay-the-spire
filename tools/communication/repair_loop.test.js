#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const {
  archiveAttempt,
  compareWorkspaceTests,
  fingerprintFor,
  isInfrastructureFailure,
  isUnsupportedBoss,
  parseCargoTestFailures,
  parseArgs,
  shouldContinue,
} = require("./repair_loop");

function tempRoot(name) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `sts-repair-loop-${name}-`));
}

function testArgumentParsing() {
  const options = parseArgs([
    "--workers",
    "2",
    "--runs",
    "9",
    "--bridge",
    "bridge-a",
    "--bridge",
    "bridge-b",
    "--bridge-session-dir",
    "/tmp/bridge-a",
    "--bridge-session-dir",
    "/tmp/bridge-b",
    "--repair-agent",
    "queue",
    "--source-version",
    "test-build",
    "--",
    "--ascension",
    "0",
    "--victory",
  ]);
  assert.equal(options.workers, 2);
  assert.equal(options.runs, 9);
  assert.deepEqual(options.bridges, ["bridge-a", "bridge-b"]);
  assert.deepEqual(options.bridgeSessionDirs, ["/tmp/bridge-a", "/tmp/bridge-b"]);
  assert.deepEqual(options.collectArgs, [
    "--ascension",
    "0",
    "--victory",
    "--combat-search-time-budget-ms",
    "250",
  ]);
  assert.equal(options.sourceVersion, "test-build");
  assert.equal(options.bossRepairAgent, "codex");
}

function testCollectionIsIndefiniteByDefault() {
  const options = parseArgs(["--bridge", "communication-mod"]);
  assert.equal(options.runs, null);
}

function testExplicitSearchBudgetWins() {
  const options = parseArgs([
    "--",
    "--ascension",
    "0",
    "--combat-search-time-budget-ms",
    "250",
  ]);
  assert.equal(
    options.collectArgs.filter((arg) => arg === "--combat-search-time-budget-ms").length,
    1,
  );
  assert.equal(options.collectArgs.at(-1), "250");
}

function testFingerprintDeduplicatesRunIdentity() {
  const first = fingerprintFor({
    blocker_kind: "simulator_fidelity_break",
    first_simulator_diff_or_mapping_failure: "monster intent differs at step 101 for run 100001",
  });
  const second = fingerprintFor({
    blocker_kind: "simulator_fidelity_break",
    first_simulator_diff_or_mapping_failure: "monster intent differs at step 202 for run 200002",
  });
  assert.equal(first, second);
}

function testArchiveIsImmutableAndTasksDeduplicate() {
  const root = tempRoot("archive");
  const trace = path.join(root, "source.jsonl");
  fs.writeFileSync(trace, '{"type":"state"}\n');
  const packet = {
    blocker_kind: "simulator_fidelity_break",
    session_id: "session-7",
    trace_path: trace,
    first_simulator_diff_or_mapping_failure: "monster intent differs at step 101 for run 100001",
    reproduce_command: "sts_verify parity source.jsonl",
  };
  const first = archiveAttempt({
    outputRoot: root,
    workerId: "worker-1",
    attemptId: 1,
    packet,
    result: { status: "simulator_mismatch", blocker_kind: packet.blocker_kind },
    sourceVersion: "version-a",
  });
  const second = archiveAttempt({
    outputRoot: root,
    workerId: "worker-1",
    attemptId: 2,
    packet: {
      ...packet,
      session_id: "session-8",
      first_simulator_diff_or_mapping_failure: "monster intent differs at step 202 for run 200002",
    },
    result: { status: "simulator_mismatch", blocker_kind: packet.blocker_kind },
    sourceVersion: "version-a",
  });
  assert.equal(first.fingerprint, second.fingerprint);
  assert.equal(second.task.occurrence_count, 2);
  assert.equal(second.task.archived_traces.length, 2);
  assert.equal(fs.readFileSync(trace, "utf8"), '{"type":"state"}\n');
  assert.equal(fs.existsSync(first.archivedTracePath), true);
  assert.equal(fs.existsSync(second.archivedTracePath), true);
  fs.rmSync(root, { recursive: true, force: true });
}

function testCompletedAttemptsAreNotRepairs() {
  const root = tempRoot("completed");
  const result = archiveAttempt({
    outputRoot: root,
    workerId: "worker-1",
    attemptId: 1,
    packet: {
      blocker_kind: "completed_trace",
      trace_path: null,
      first_simulator_diff_or_mapping_failure: null,
    },
    result: { status: "completed_trace", blocker_kind: "completed_trace" },
    sourceVersion: "version-a",
  });
  assert.equal(result.kind, "completed_trace");
  assert.equal(fs.existsSync(path.join(root, "tasks")), false);
  fs.rmSync(root, { recursive: true, force: true });
}

function testArchivePathsNeverOverwriteOnRestart() {
  const root = tempRoot("restart");
  const trace = path.join(root, "source.jsonl");
  const packet = {
    blocker_kind: "simulator_fidelity_break",
    session_id: "session-1",
    trace_path: trace,
    first_simulator_diff_or_mapping_failure: "unsupported transition",
  };
  fs.writeFileSync(trace, '{"version":1}\n');
  const first = archiveAttempt({
    outputRoot: root,
    workerId: "worker-1",
    attemptId: 1,
    packet,
    result: { blocker_kind: packet.blocker_kind },
    sourceVersion: "version-a",
  });
  fs.writeFileSync(trace, '{"version":2}\n');
  const restarted = archiveAttempt({
    outputRoot: root,
    workerId: "worker-1",
    attemptId: 1,
    packet,
    result: { blocker_kind: packet.blocker_kind },
    sourceVersion: "version-b",
  });

  assert.notEqual(first.archivedTracePath, restarted.archivedTracePath);
  assert.equal(fs.readFileSync(first.archivedTracePath, "utf8"), '{"version":1}\n');
  assert.equal(fs.readFileSync(restarted.archivedTracePath, "utf8"), '{"version":2}\n');
  fs.rmSync(root, { recursive: true, force: true });
}

function testContinuationPolicy() {
  assert.equal(shouldContinue({ blocker_kind: "simulator_fidelity_break" }, null), true);
  assert.equal(shouldContinue({ blocker_kind: "slaythedata_mapping_gap" }, null), true);
  assert.equal(shouldContinue({ blocker_kind: "bridge_or_backend_error" }, null), false);
  assert.equal(shouldContinue({ status: "no_candidates" }, null), false);
  assert.equal(
    shouldContinue(
      { status: "blocked", blocker_kind: "slaythedata_mapping_gap", reason: "slaythedata_search_failed" },
      null,
    ),
    false,
  );
}

function testInfrastructureFailuresUseDurableRetryLane() {
  assert.equal(
    isInfrastructureFailure(
      { blocker_kind: "bridge_or_backend_error", reason: "reset_bridge_failed" },
      null,
    ),
    true,
  );
  assert.equal(
    isInfrastructureFailure({ blocker_kind: "simulator_fidelity_break" }, null),
    false,
  );
}

function testOnlyUnsupportedBossesUseSynchronousLane() {
  const unsupported = {
    blocker_kind: "simulator_fidelity_break",
    strict_verification: { unsupported_actions: 1 },
  };
  const bossPacket = {
    current_live_state_summary: {
      phase: "combat",
      floor: 17,
      summary: { room_type: "MonsterRoomBoss" },
    },
    first_simulator_diff_or_mapping_failure: "unsupported boss transition",
  };
  assert.equal(isUnsupportedBoss(unsupported, bossPacket), true);
  assert.equal(
    isUnsupportedBoss(unsupported, {
      ...bossPacket,
      current_live_state_summary: {
        phase: "combat",
        floor: 8,
        summary: { room_type: "MonsterRoom" },
      },
    }),
    false,
  );
  assert.equal(
    isUnsupportedBoss(
      {
        blocker_kind: "simulator_fidelity_break",
        strict_verification: { unsupported_actions: 0 },
      },
      {
        ...bossPacket,
        first_simulator_diff_or_mapping_failure: "monster hp mismatch",
      },
    ),
    false,
  );
}

function testWorkspaceRegressionGateAllowsOnlyBaselineFailures() {
  const output = `
failures:

---- tests::first stdout ----
panic details

failures:
    tests::first
    tests::second

test result: FAILED
`;
  assert.deepEqual(parseCargoTestFailures(output), ["tests::first", "tests::second"]);
  assert.equal(
    compareWorkspaceTests(
      { comparable: true, ok: false, failures: ["tests::first", "tests::second"] },
      { comparable: true, ok: false, failures: ["tests::second"] },
    ).ok,
    true,
  );
  const regression = compareWorkspaceTests(
    { comparable: true, ok: false, failures: ["tests::first"] },
    { comparable: true, ok: false, failures: ["tests::first", "tests::new"] },
  );
  assert.equal(regression.ok, false);
  assert.deepEqual(regression.new_failures, ["tests::new"]);
}

testArgumentParsing();
testCollectionIsIndefiniteByDefault();
testExplicitSearchBudgetWins();
testFingerprintDeduplicatesRunIdentity();
testArchiveIsImmutableAndTasksDeduplicate();
testCompletedAttemptsAreNotRepairs();
testArchivePathsNeverOverwriteOnRestart();
testContinuationPolicy();
testInfrastructureFailuresUseDurableRetryLane();
testOnlyUnsupportedBossesUseSynchronousLane();
testWorkspaceRegressionGateAllowsOnlyBaselineFailures();
console.log("repair_loop tests passed");
