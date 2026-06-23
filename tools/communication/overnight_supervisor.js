#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const childProcess = require("child_process");

const repoRoot = path.resolve(__dirname, "..", "..");
const sessionDir = path.join(__dirname, "session");
const statusPath = path.join(sessionDir, "status.json");
const summaryPath = path.join(sessionDir, "summary.json");
const logPath = path.join(sessionDir, "overnight_supervisor.log");
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
    return false;
  }
  const result = childProcess.spawnSync(nodeExe, [traceToolsPath, "validate", tracePath], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const output = `${result.stdout || ""}${result.stderr || ""}`.trim();
  if (output) log(`trace validation for ${tracePath}:\n${output}`);
  return result.status === 0;
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
      process.exitCode = 2;
      return;
    }

    const beforeTrace = currentTracePath();
    log(`starting collector restart=${restart + 1}/${maxRestarts} trace=${beforeTrace || "unknown"}`);
    const collector = startCollector();
    const result = await waitForCollector(collector);
    const afterTrace = currentTracePath() || beforeTrace;
    log(`collector exited code=${result.code} signal=${result.signal || ""}`);
    validateTrace(afterTrace);

    const afterStale = bridgeLooksStale();
    if (afterStale.stale) {
      log(`stopping supervisor: ${afterStale.reason}`);
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
  bridgeLooksStaleFrom,
  currentTracePathFromStatus,
};
