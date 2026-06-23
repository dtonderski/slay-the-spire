#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const childProcess = require("child_process");

const repoRoot = path.resolve(__dirname, "..", "..");
const sessionDir = path.join(__dirname, "session");
const statusPath = path.join(sessionDir, "status.json");
const summaryPath = path.join(sessionDir, "summary.json");
const logPath = path.join(sessionDir, "overnight_supervisor.log");
const harvestReportPath = path.join(sessionDir, "harvest_report.json");
const collectorPath = path.join(__dirname, "overnight_collector.js");
const traceToolsPath = path.join(__dirname, "trace_tools.js");

const nodeExe = process.execPath;
const maxRestarts = Number.parseInt(process.env.STS_SUPERVISOR_MAX_RESTARTS || "20", 10);
const staleMs = Number.parseInt(process.env.STS_SUPERVISOR_STALE_MS || "120000", 10);
const restartDelayMs = Number.parseInt(process.env.STS_SUPERVISOR_RESTART_DELAY_MS || "3000", 10);

fs.mkdirSync(sessionDir, { recursive: true });

function log(line) {
  const message = `[${new Date().toISOString()}] ${line}`;
  console.log(message);
  fs.appendFileSync(logPath, `${message}\n`);
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function fileAgeMs(filePath) {
  try {
    return Date.now() - fs.statSync(filePath).mtimeMs;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function currentTracePathFromStatus(status) {
  if (typeof status?.trace_path === "string") return status.trace_path;
  return null;
}

function currentTracePath() {
  return currentTracePathFromStatus(readJson(statusPath));
}

function bridgeLooksStaleFrom({ summary, status, summaryAgeMs, statusAgeMs, staleThresholdMs }) {
  if (!summary && !status) {
    return { stale: true, reason: "no session summary/status files" };
  }
  if (summaryAgeMs > staleThresholdMs && statusAgeMs > staleThresholdMs) {
    return {
      stale: true,
      reason: `session files stale: summaryAgeMs=${Math.round(summaryAgeMs)} statusAgeMs=${Math.round(statusAgeMs)}`,
    };
  }
  if (status?.status === "exited") {
    return { stale: true, reason: `bridge exited: ${status.reason || "unknown"}` };
  }
  return { stale: false, reason: "session active" };
}

function bridgeLooksStale() {
  return bridgeLooksStaleFrom({
    summary: readJson(summaryPath),
    status: readJson(statusPath),
    summaryAgeMs: fileAgeMs(summaryPath),
    statusAgeMs: fileAgeMs(statusPath),
    staleThresholdMs: staleMs,
  });
}

function validateTrace(tracePath) {
  if (!tracePath || !fs.existsSync(tracePath)) {
    log(`no trace to validate: ${tracePath || "unknown"}`);
    return { ok: false, result: null };
  }
  const result = childProcess.spawnSync(nodeExe, [traceToolsPath, "validate", tracePath], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const output = `${result.stdout || ""}${result.stderr || ""}`.trim();
  if (output) log(`trace validation for ${tracePath}:\n${output}`);
  const parsed = parseValidationOutput(result.stdout);
  if (parsed?.summary) log(formatValidationSummary(parsed.summary));
  return { ok: result.status === 0, result: parsed };
}

function reportTrace(tracePath) {
  if (!tracePath || !fs.existsSync(tracePath)) return null;
  const result = childProcess.spawnSync(nodeExe, [traceToolsPath, "report", tracePath], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const parsed = parseValidationOutput(result.stdout);
  if (parsed?.best_run) log(formatBestRunSummary(parsed.best_run));
  return parsed;
}

function bestRunPath(tracePath) {
  const parsed = path.parse(tracePath);
  return path.join(parsed.dir, `${parsed.name}.best-run${parsed.ext || ".jsonl"}`);
}

function extractBestRunTrace(tracePath) {
  if (!tracePath || !fs.existsSync(tracePath)) return { ok: false, destination: null };
  const destination = bestRunPath(tracePath);
  if (fs.existsSync(destination)) {
    const existing = validateTrace(destination);
    if (existing.ok) {
      log(`existing best-run trace is already valid: ${destination}`);
      return { ok: true, destination, reused: true };
    }
  }
  const result = childProcess.spawnSync(nodeExe, [traceToolsPath, "extract-best-run", tracePath, destination], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const output = `${result.stdout || ""}${result.stderr || ""}`.trim();
  if (output) log(`best-run extract for ${tracePath} -> ${destination}:\n${output}`);
  return { ok: result.status === 0, destination, reused: false };
}

function reportAndExtractBestRun(tracePath) {
  const report = reportTrace(tracePath);
  const extraction = report?.best_run ? extractBestRunTrace(tracePath) : null;
  return { report, extraction };
}

function validPrefixPath(tracePath) {
  const parsed = path.parse(tracePath);
  return path.join(parsed.dir, `${parsed.name}.valid-prefix${parsed.ext || ".jsonl"}`);
}

function trimValidPrefix(tracePath) {
  const destination = validPrefixPath(tracePath);
  if (fs.existsSync(destination)) {
    const existing = validateTrace(destination);
    if (existing.ok) {
      log(`existing valid-prefix trace is already valid: ${destination}`);
      return { ok: true, destination, reused: true, output: "" };
    }
  }
  const result = childProcess.spawnSync(nodeExe, [traceToolsPath, "trim-valid-prefix", tracePath, destination], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const output = `${result.stdout || ""}${result.stderr || ""}`.trim();
  if (output) log(`trace trim for ${tracePath} -> ${destination}:\n${output}`);
  return { ok: result.status === 0, destination, output };
}

function validateOrTrimTrace(tracePath) {
  const validation = validateTrace(tracePath);
  if (validation.ok) {
    const bestRun = reportAndExtractBestRun(tracePath);
    return { trace_path: tracePath, validation, trimmed: null, best_run: bestRun };
  }
  if (!tracePath || !fs.existsSync(tracePath)) {
    return { trace_path: tracePath || null, validation, trimmed: null, best_run: null };
  }
  const trimmed = trimValidPrefix(tracePath);
  let bestRun = null;
  if (trimmed.ok) {
    if (!trimmed.reused) validateTrace(trimmed.destination);
    bestRun = reportAndExtractBestRun(trimmed.destination);
  }
  return { trace_path: tracePath, validation, trimmed, best_run: bestRun };
}

function parseValidationOutput(output) {
  if (!output || !output.trim()) return null;
  try {
    return JSON.parse(output);
  } catch {
    return null;
  }
}

function formatValidationSummary(summary) {
  const terminal = summary.terminal || {};
  const coverage = summary.coverage || {};
  return [
    "trace harvest:",
    `actions=${summary.actions ?? "?"}`,
    `maxFloor=${summary.max_floor ?? "?"}`,
    `elites=${summary.elite_rooms ?? 0}`,
    `bosses=${summary.boss_rooms ?? 0}`,
    `deaths=${summary.deaths ?? 0}`,
    `terminal=${terminal.kind || "unknown"}`,
    `score=${coverage.score ?? "?"}`,
  ].join(" ");
}

function formatBestRunSummary(run) {
  const summary = run.validation?.summary || {};
  const terminal = summary.terminal || {};
  const coverage = summary.coverage || {};
  return [
    "best run:",
    `index=${run.run_index}`,
    `startStep=${run.start_step}`,
    `actions=${summary.actions ?? "?"}`,
    `maxFloor=${summary.max_floor ?? "?"}`,
    `elites=${summary.elite_rooms ?? 0}`,
    `bosses=${summary.boss_rooms ?? 0}`,
    `deaths=${summary.deaths ?? 0}`,
    `terminal=${terminal.kind || "unknown"}`,
    `score=${coverage.score ?? "?"}`,
  ].join(" ");
}

function compactValidation(validation) {
  const summary = validation?.result?.summary || {};
  return {
    ok: validation?.ok ?? false,
    missing: validation?.result?.missing || [],
    actions: summary.actions ?? null,
    max_floor: summary.max_floor ?? null,
    elite_rooms: summary.elite_rooms ?? 0,
    boss_rooms: summary.boss_rooms ?? 0,
    deaths: summary.deaths ?? 0,
    terminal: summary.terminal || null,
    score: summary.coverage?.score ?? null,
    seeds: summary.seeds || [],
    act_bosses: summary.act_bosses || [],
    encounters: summary.encounters || [],
  };
}

function compactBestRun(bestRun) {
  const run = bestRun?.report?.best_run;
  if (!run) return null;
  const summary = run.validation?.summary || {};
  return {
    run_index: run.run_index,
    start_step: run.start_step,
    command: run.command,
    extracted_path: bestRun.extraction?.destination || null,
    extracted_reused: bestRun.extraction?.reused ?? false,
    actions: summary.actions ?? null,
    max_floor: summary.max_floor ?? null,
    elite_rooms: summary.elite_rooms ?? 0,
    boss_rooms: summary.boss_rooms ?? 0,
    deaths: summary.deaths ?? 0,
    terminal: summary.terminal || null,
    score: summary.coverage?.score ?? null,
    encounters: summary.encounters || [],
  };
}

function buildHarvestReport({ reason, traceResult, stale, collectorResult }) {
  return {
    updated_at: new Date().toISOString(),
    reason,
    stale: stale || null,
    collector: collectorResult || null,
    trace_path: traceResult?.trace_path || null,
    validation: compactValidation(traceResult?.validation),
    valid_prefix_path: traceResult?.trimmed?.ok ? traceResult.trimmed.destination : null,
    valid_prefix_reused: traceResult?.trimmed?.reused ?? false,
    best_run: compactBestRun(traceResult?.best_run),
  };
}

function writeHarvestReport(input) {
  const report = buildHarvestReport(input);
  writeJson(harvestReportPath, report);
  log(`harvest report: ${harvestReportPath}`);
  return report;
}

function startCollector() {
  const child = childProcess.spawn(nodeExe, [collectorPath], {
    cwd: repoRoot,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.stdout.on("data", (chunk) => {
    for (const line of String(chunk).trimEnd().split(/\r?\n/).filter(Boolean)) {
      log(`collector stdout: ${line}`);
    }
  });
  child.stderr.on("data", (chunk) => {
    for (const line of String(chunk).trimEnd().split(/\r?\n/).filter(Boolean)) {
      log(`collector stderr: ${line}`);
    }
  });
  return child;
}

async function waitForCollector(child) {
  return new Promise((resolve) => {
    child.on("exit", (code, signal) => resolve({ code, signal }));
  });
}

async function main() {
  log(`overnight supervisor started at ${repoRoot}`);
  for (let restart = 0; restart < maxRestarts; restart += 1) {
    const stale = bridgeLooksStale();
    if (stale.stale) {
      log(`cannot start collector: ${stale.reason}`);
      const traceResult = validateOrTrimTrace(currentTracePath());
      writeHarvestReport({ reason: "stale_before_start", stale, traceResult });
      process.exitCode = 2;
      return;
    }

    const beforeTrace = currentTracePath();
    log(`starting collector restart=${restart + 1}/${maxRestarts} trace=${beforeTrace || "unknown"}`);
    const collector = startCollector();
    const result = await waitForCollector(collector);
    const afterTrace = currentTracePath() || beforeTrace;
    log(`collector exited code=${result.code} signal=${result.signal || ""}`);
    const traceResult = validateOrTrimTrace(afterTrace);
    writeHarvestReport({ reason: "collector_exit", collectorResult: result, traceResult });

    const afterStale = bridgeLooksStale();
    if (afterStale.stale) {
      log(`stopping supervisor: ${afterStale.reason}`);
      writeHarvestReport({ reason: "stale_after_collector", stale: afterStale, collectorResult: result, traceResult });
      process.exitCode = result.code || 2;
      return;
    }
    await sleep(restartDelayMs);
  }
  log(`max supervisor restarts reached: ${maxRestarts}`);
}

if (require.main === module) {
  main().catch((error) => {
    log(`supervisor failed: ${error.stack || error.message}`);
    process.exitCode = 1;
  });
}

module.exports = {
  bestRunPath,
  buildHarvestReport,
  bridgeLooksStaleFrom,
  compactBestRun,
  compactValidation,
  currentTracePathFromStatus,
  extractBestRunTrace,
  formatBestRunSummary,
  formatValidationSummary,
  parseValidationOutput,
  validPrefixPath,
};
