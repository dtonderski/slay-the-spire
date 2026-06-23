#!/usr/bin/env node

const assert = require("assert");
const {
  bestRunPath,
  buildHarvestReport,
  bridgeLooksStaleFrom,
  compactBestRun,
  compactValidation,
  currentTracePathFromStatus,
  formatBestRunSummary,
  formatValidationSummary,
  parseValidationOutput,
  validPrefixPath,
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

function testValidationOutputParsing() {
  const parsed = parseValidationOutput('{"ok":true,"summary":{"actions":3}}');
  assert.strictEqual(parsed.ok, true);
  assert.strictEqual(parsed.summary.actions, 3);
  assert.strictEqual(parseValidationOutput("not json"), null);
}

function testValidationSummaryFormatting() {
  const line = formatValidationSummary({
    actions: 42,
    max_floor: 7,
    elite_rooms: 1,
    boss_rooms: 0,
    deaths: 0,
    terminal: { kind: "reward_screen" },
    coverage: { score: 142 },
  });
  assert.match(line, /actions=42/);
  assert.match(line, /maxFloor=7/);
  assert.match(line, /elites=1/);
  assert.match(line, /terminal=reward_screen/);
  assert.match(line, /score=142/);
}

function testValidPrefixPath() {
  assert.strictEqual(
    validPrefixPath("D:\\dev\\slay-the-spire\\verification\\corpus\\communication_mod\\trace-a.jsonl"),
    "D:\\dev\\slay-the-spire\\verification\\corpus\\communication_mod\\trace-a.valid-prefix.jsonl",
  );
}

function testBestRunPath() {
  assert.strictEqual(
    bestRunPath("D:\\dev\\slay-the-spire\\verification\\corpus\\communication_mod\\trace-a.valid-prefix.jsonl"),
    "D:\\dev\\slay-the-spire\\verification\\corpus\\communication_mod\\trace-a.valid-prefix.best-run.jsonl",
  );
}

function testBestRunSummaryFormatting() {
  const line = formatBestRunSummary({
    run_index: 1,
    start_step: 87,
    validation: {
      summary: {
        actions: 225,
        max_floor: 8,
        elite_rooms: 1,
        boss_rooms: 0,
        deaths: 0,
        terminal: { kind: "reward_screen" },
        coverage: { score: 335 },
      },
    },
  });
  assert.match(line, /index=1/);
  assert.match(line, /startStep=87/);
  assert.match(line, /elites=1/);
  assert.match(line, /score=335/);
}

function testCompactValidation() {
  const compact = compactValidation({
    ok: true,
    result: {
      missing: [],
      summary: {
        actions: 323,
        max_floor: 8,
        elite_rooms: 1,
        deaths: 0,
        terminal: { kind: "reward_screen" },
        coverage: { score: 433 },
        seeds: [40560393126],
        encounters: ["Sentry+Sentry+Sentry"],
      },
    },
  });
  assert.strictEqual(compact.ok, true);
  assert.strictEqual(compact.actions, 323);
  assert.strictEqual(compact.elite_rooms, 1);
  assert.deepStrictEqual(compact.encounters, ["Sentry+Sentry+Sentry"]);
}

function testBuildHarvestReportIncludesBestRunArtifact() {
  const report = buildHarvestReport({
    reason: "collector_exit",
    collectorResult: { code: 2, signal: null },
    traceResult: {
      trace_path: "trace.jsonl",
      validation: { ok: true, result: { missing: [], summary: { actions: 10, coverage: { score: 20 } } } },
      trimmed: { ok: true, destination: "trace.valid-prefix.jsonl", reused: true },
      best_run: {
        extraction: { destination: "trace.best-run.jsonl", reused: false },
        report: {
          best_run: {
            run_index: 1,
            start_step: 87,
            command: "START IRONCLAD 0 M290001",
            validation: {
              summary: {
                actions: 323,
                max_floor: 8,
                elite_rooms: 1,
                deaths: 0,
                terminal: { kind: "reward_screen" },
                coverage: { score: 433 },
                encounters: ["Sentry+Sentry+Sentry"],
              },
            },
          },
        },
      },
    },
  });
  assert.strictEqual(report.reason, "collector_exit");
  assert.strictEqual(report.valid_prefix_path, "trace.valid-prefix.jsonl");
  assert.strictEqual(report.best_run.extracted_path, "trace.best-run.jsonl");
  assert.strictEqual(report.best_run.max_floor, 8);
}

function testCompactBestRunHandlesMissingRun() {
  assert.strictEqual(compactBestRun(null), null);
}

testNoSessionFilesAreStale();
testOldSessionFilesAreStale();
testExitedBridgeIsStale();
testFreshSessionIsActive();
testTracePathExtraction();
testValidationOutputParsing();
testValidationSummaryFormatting();
testValidPrefixPath();
testBestRunPath();
testBestRunSummaryFormatting();
testCompactValidation();
testBuildHarvestReportIncludesBestRunArtifact();
testCompactBestRunHandlesMissingRun();

console.log("overnight_supervisor tests passed");
