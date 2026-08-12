#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

const root = path.resolve(__dirname, "..", "..");
const simulatorDir = path.join(root, "simulator");
const outputDir = path.resolve(
  process.env.STS_RANDOM_OUTPUT_DIR ||
    path.join(root, "simulator", "target", "random-fidelity"),
);
function externalCorpusDir() {
  const configured = process.env.STS_PERMANENT_CORPUS_DIR;
  if (!configured) {
    throw new Error("STS_PERMANENT_CORPUS_DIR must name the external trace directory");
  }
  return path.resolve(configured);
}
const checkpointsPath = path.join(outputDir, "promotion_checkpoints.jsonl");
const latestLogPath = path.join(outputDir, "promotion_gate_latest.log");

function parseFailureTraceNames(output) {
  return [...new Set(
    [...String(output).matchAll(/^([^:\r\n]+\.jsonl): /gm)]
      .map((match) => match[1])
      .filter((name) => path.basename(name) === name),
  )];
}

function run(command, args, options) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      ...options,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.once("error", reject);
    child.once("close", (code, signal) => {
      resolve({ code, signal, stdout, stderr });
    });
  });
}

function reconcileVerifiedPrefixes(entries) {
  const {
    satisfyTaskByFingerprint,
  } = require("./random_fidelity_repair_queue");
  const tasksDir = path.join(outputDir, "repair_tasks");
  let resolved = 0;
  for (const entry of entries) {
    const originalFingerprint = entry.trace?.match(
      /^random-fidelity-([0-9a-f]{16})\.jsonl$/,
    )?.[1];
    if (entry.status !== "verified_prefix" || !originalFingerprint) continue;
    const before = fs.existsSync(path.join(tasksDir, originalFingerprint, "task.json"))
      ? JSON.parse(
        fs.readFileSync(path.join(tasksDir, originalFingerprint, "task.json"), "utf8"),
      ).status
      : null;
    satisfyTaskByFingerprint(
      tasksDir,
      originalFingerprint,
      "authoritative corpus gate verified this retained prefix",
    );
    if (before === "queued") resolved += 1;
  }
  return resolved;
}

function enqueueFailedTraces(traceNames) {
  process.env.STS_RANDOM_OUTPUT_DIR = outputDir;
  const { verifyEntry } = require("./random_fidelity_verifier");
  const corpusDir = externalCorpusDir();
  const checked = traceNames.flatMap((traceName) => {
    const trace = path.join(corpusDir, traceName);
    if (!fs.existsSync(trace)) return [];
    const result = verifyEntry(
      {
        trace,
        game_seed: `CORPUS-${path.basename(traceName, ".jsonl")}`,
        policy_seed: 0,
        actions: null,
      },
      {
        // Permanent witnesses are intentionally minimized and commonly end
        // before a terminal game state. A clean verified prefix is not a new
        // simulator divergence and must not create a synthetic repair task.
        acceptVerifiedPrefix: true,
      },
    );
    return [{
      trace: traceName,
      status: result.status,
      fingerprint: result.fingerprint,
      boundary: result.boundary,
    }];
  });
  reconcileVerifiedPrefixes(checked);
  return checked;
}

function appendCheckpoint(value) {
  fs.mkdirSync(outputDir, { recursive: true });
  fs.appendFileSync(checkpointsPath, `${JSON.stringify(value)}\n`);
}

async function main() {
  if (process.argv.includes("--reconcile-latest")) {
    const checkpoints = fs
      .readFileSync(checkpointsPath, "utf8")
      .split(/\r?\n/)
      .filter(Boolean);
    const latest = JSON.parse(checkpoints.at(-1));
    const resolved = reconcileVerifiedPrefixes(latest.enqueued || []);
    process.stdout.write(`${JSON.stringify({ resolved })}\n`);
    return;
  }
  const command = "uv";
  const args = [
    "run",
    "--python",
    "3.12",
    "cargo",
    "test",
    "-p",
    "sts_verify",
    "--test",
    "corpus",
    "external_permanent_traces_are_complete_passes",
    "--",
    "--ignored",
    "--nocapture",
  ];
  const result = await run(command, args, {
    cwd: simulatorDir,
    env: {
      ...process.env,
      UV_CACHE_DIR: process.env.UV_CACHE_DIR || "/tmp/sts-uv-cache",
    },
  });
  const output = `${result.stdout}\n${result.stderr}`;
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(latestLogPath, output);
  const traceNames = parseFailureTraceNames(output);
  const enqueued = result.code === 0 ? [] : enqueueFailedTraces(traceNames);
  const checkpoint = {
    checked_at: new Date().toISOString(),
    gate: "external_permanent_traces_are_complete_passes",
    status: result.code === 0 ? "passed" : "failed",
    failed_traces: traceNames.length,
    promotion: result.code === 0,
    enqueued,
    exit_code: result.code,
    signal: result.signal,
  };
  appendCheckpoint(checkpoint);
  const consoleCheckpoint = {
    ...checkpoint,
    enqueued: {
      checked: enqueued.length,
      divergences: enqueued.filter((entry) => entry.status === "divergence").length,
      verified_prefixes: enqueued.filter((entry) => entry.status === "verified_prefix").length,
      integrity_errors: enqueued.filter((entry) => entry.status === "trace_integrity_error").length,
    },
    log: latestLogPath,
  };
  process.stdout.write(`${JSON.stringify(consoleCheckpoint, null, 2)}\n`);
  if (result.code !== 0) {
    process.stderr.write(
      `external corpus gate failed; full output: ${latestLogPath}\n`,
    );
    process.exitCode = result.code || 1;
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exit(1);
  });
}

module.exports = {
  enqueueFailedTraces,
  parseFailureTraceNames,
  reconcileVerifiedPrefixes,
};
