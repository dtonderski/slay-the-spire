#!/usr/bin/env node

const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const { spawnSync } = require("child_process");

const root = path.resolve(__dirname, "..", "..");
const defaultTraceRoot = path.join(root, "random_traces_loop");
const defaultVerifier = path.join(root, "simulator", "target", "release", "sts_verify");

function parseArgs(argv) {
  const options = {
    traceRoot: defaultTraceRoot,
    verifier: defaultVerifier,
    maxTraces: null,
    maxRepairAttempts: Infinity,
    repairAgents: 3,
    repairTimeoutMs: 15 * 60 * 1000,
    noRepair: false,
    retryBlocked: true,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => {
      index += 1;
      if (index >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[index];
    };
    if (arg === "--trace-root") options.traceRoot = path.resolve(next());
    else if (arg === "--verifier") options.verifier = path.resolve(next());
    else if (arg === "--max-traces") options.maxTraces = positiveInteger(next(), arg);
    else if (arg === "--max-repair-attempts") options.maxRepairAttempts = positiveInteger(next(), arg);
    else if (arg === "--repair-agents") options.repairAgents = positiveInteger(next(), arg);
    else if (arg === "--repair-timeout-ms") options.repairTimeoutMs = positiveInteger(next(), arg);
    else if (arg === "--no-repair") options.noRepair = true;
    else if (arg === "--retry-blocked") options.retryBlocked = true;
    else if (arg === "--skip-blocked") options.retryBlocked = false;
    else if (arg === "--help") options.help = true;
    else throw new Error(`unknown option ${arg}`);
  }
  return options;
}

function positiveInteger(value, flag) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 1) throw new Error(`${flag} requires a positive integer`);
  return parsed;
}

function traceFiles(traceRoot) {
  const directory = path.join(traceRoot, "traces");
  return fs.readdirSync(directory)
    .filter((name) => name.endsWith(".jsonl"))
    .sort()
    .map((name) => path.join(directory, name));
}

function parseVerifierOutput(output, exitCode) {
  const value = String(output || "");
  const outcome = value.match(/^outcome=(\S+)/m)?.[1] || null;
  const boundary = {
    path: value.match(/^seed_start\.first_boundary\.path=(.*)$/m)?.[1] || null,
    category: value.match(/^seed_start\.first_boundary\.category=(.*)$/m)?.[1] || null,
    reason: value.match(/^seed_start\.first_boundary\.reason=(.*)$/m)?.[1] || null,
  };
  if (boundary.category === "none") {
    boundary.path = null;
    boundary.reason = null;
  }
  return {
    ok: outcome === "complete_pass",
    outcome,
    exitCode,
    boundary: boundary.category ? boundary : null,
    totalActions: Number(value.match(/^total_actions=(\d+)$/m)?.[1] || 0),
    verified: Number(value.match(/^verified=(\d+)$/m)?.[1] || 0),
    unsupported: Number(value.match(/^unsupported=(\d+)$/m)?.[1] || 0),
    unexpectedDiffs: Number(value.match(/^unexpected_diffs=(\d+)$/m)?.[1] || 0),
    raw: value,
  };
}

function verifyTrace(verifier, tracePath, logPath) {
  const result = spawnSync(verifier, ["parity", tracePath], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  const output = `${result.stdout || ""}${result.stderr || ""}`;
  fs.mkdirSync(path.dirname(logPath), { recursive: true });
  fs.writeFileSync(logPath, output);
  if (result.error) {
    return { ok: false, outcome: "verifier_error", exitCode: null, error: result.error.message, raw: output };
  }
  return parseVerifierOutput(output, result.status);
}

function traceFingerprint(tracePath) {
  return crypto.createHash("sha256").update(fs.readFileSync(tracePath)).digest("hex").slice(0, 16);
}

function corpusPaths() {
  const corpus = path.join(root, "simulator", "verification", "corpus");
  return {
    directory: corpus,
    traces: path.join(corpus, "permanent_traces"),
    openFailures: path.join(corpus, "open_failures"),
  };
}

function writeJsonAtomic(filePath, value) {
  const temporary = `${filePath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
  fs.renameSync(temporary, filePath);
}

function permanentizeFailure(tracePath, verification) {
  if (!verification.boundary?.path || !verification.boundary?.category || verification.boundary.category === "invalid_input") {
    throw new Error(`cannot permanentize trace without a replay boundary: ${tracePath}`);
  }
  const fingerprint = traceFingerprint(tracePath);
  const traceName = `random-fidelity-${fingerprint}.jsonl`;
  const { openFailures } = corpusPaths();
  // Keep full failing witnesses outside the green permanent_traces gate.
  fs.mkdirSync(openFailures, { recursive: true });
  const destination = path.join(openFailures, traceName);
  if (!fs.existsSync(destination)) fs.copyFileSync(tracePath, destination);
  return { fingerprint, traceName, destination };
}

function markPermanentTraceComplete(traceName) {
  // No expectation manifest: green corpus is directory membership + clean EOF.
  // Callers may copy a clean prefix into permanent_traces separately.
  void traceName;
}

function appendState(traceRoot, event) {
  const statePath = path.join(traceRoot, "serial_processor_state.jsonl");
  fs.mkdirSync(path.dirname(statePath), { recursive: true });
  fs.appendFileSync(statePath, `${JSON.stringify({ at: new Date().toISOString(), ...event })}\n`);
}

function appendWorkerState(traceRoot, worker, event) {
  appendState(traceRoot, { worker, ...event });
}

function loadState(traceRoot) {
  const statePath = path.join(traceRoot, "serial_processor_state.jsonl");
  const removed = new Set();
  const blocked = new Set();
  if (!fs.existsSync(statePath)) return { removed, blocked };
  for (const line of fs.readFileSync(statePath, "utf8").split("\n")) {
    if (!line.trim()) continue;
    try {
      const event = JSON.parse(line);
      if (event.trace && (event.event === "passed_removed" || event.event === "repaired_removed")) {
        removed.add(event.trace);
        blocked.delete(event.trace);
      } else if (event.trace && (event.event === "blocked" || event.event === "repair_blocked")) {
        if (!removed.has(event.trace)) blocked.add(event.trace);
      }
    } catch {
      // A truncated final state line must not prevent resuming the corpus.
    }
  }
  return { removed, blocked };
}

function repairPrompt({ tracePath, permanentTracePath, verification, attempt, verifier }) {
  return `You are one of three independent simulator-repair agents. Repair exactly one real-game trace.

This is repair attempt ${attempt}; further attempts will continue until this trace passes. Work directly in the repository at ${root}; do not
use subagents, do not run Git state-changing commands, and do not edit trace data,
permanent_traces contents, or processor state. Read AGENT_RULES.md and docs/research.md.

Trace to reproduce: ${tracePath}
Permanent copy: ${permanentTracePath || "not yet available"}
Verifier: ${verifier}
First boundary category: ${verification.boundary?.category || "unknown"}
First boundary path: ${verification.boundary?.path || "unknown"}
First boundary reason: ${verification.boundary?.reason || "unknown"}

Use the trace and the verifier to find the first generic simulator cause. Make the
smallest source-backed deterministic fix. Never add seed-, trace-, or observation-
specific behavior, weaken comparisons, or hydrate simulator state from observations.
Run focused tests and re-run 'sts_verify parity' on this exact trace before finishing.
If this is not safely repairable, make no speculative change and explain why.\n`;
}

function runRepair(prompt, logPath, options) {
  const command = process.env.STS_SERIAL_CODEX_BIN || "codex";
  const args = [
    "exec", "--json", "--ephemeral", "--skip-git-repo-check",
    "--sandbox", "workspace-write", "-C", root,
    "-m", process.env.STS_SERIAL_REPAIR_MODEL || "gpt-5.6-luna",
    "-c", `model_reasoning_effort=\"${process.env.STS_SERIAL_REPAIR_EFFORT || "high"}\"`,
    "-c", 'approval_policy="never"', "-",
  ];
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: root,
      stdio: ["pipe", "pipe", "pipe"],
      detached: true,
    });
    const chunks = [];
    let settled = false;
    let timer = null;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      resolve({ ...result, log: writeRepairLog(logPath, chunks) });
    };
    const stopProcessGroup = () => {
      if (!child.pid) return;
      try {
        process.kill(-child.pid, "SIGTERM");
      } catch {
        child.kill("SIGTERM");
      }
    };
    child.stdout.on("data", (chunk) => chunks.push(chunk));
    child.stderr.on("data", (chunk) => chunks.push(chunk));
    child.stdin.end(prompt);
    child.on("error", (error) => finish({ code: null, error: error.message }));
    child.on("close", (code, signal) => finish({ code, signal }));
    if (options.repairTimeoutMs) {
      timer = setTimeout(stopProcessGroup, options.repairTimeoutMs);
      timer.unref();
    }
  });
}

function writeRepairLog(logPath, chunks) {
  const output = Buffer.concat(chunks).toString("utf8");
  fs.mkdirSync(path.dirname(logPath), { recursive: true });
  fs.writeFileSync(logPath, output);
  return logPath;
}

async function processTrace(tracePath, options, worker = null) {
  const name = path.basename(tracePath);
  const logPath = path.join(options.traceRoot, "serial_processor_logs", `${name}.log`);
  let verification = verifyTrace(options.verifier, tracePath, logPath);
  appendWorkerState(options.traceRoot, worker, { event: "verified", trace: name, result: summarizeVerification(verification) });
  if (verification.ok) {
    fs.unlinkSync(tracePath);
    appendWorkerState(options.traceRoot, worker, { event: "passed_removed", trace: name });
    return { status: "passed", trace: name, worker };
  }
  if (verification.outcome === "verifier_error" || !verification.boundary) {
    appendWorkerState(options.traceRoot, worker, { event: "blocked", trace: name, reason: "verifier_error_or_missing_boundary" });
    return { status: "blocked", trace: name, verification, worker };
  }
  const permanent = permanentizeFailure(tracePath, verification);
  appendWorkerState(options.traceRoot, worker, { event: "permanentized", trace: name, permanent: permanent.traceName });
  if (options.noRepair) return { status: "failed", trace: name, permanent: permanent.traceName, worker };

  let attempt = 1;
  while (options.maxRepairAttempts === Infinity || attempt <= options.maxRepairAttempts) {
    appendWorkerState(options.traceRoot, worker, { event: "repair_started", trace: name, permanent: permanent.traceName, attempt });
    const repair = await runRepair(
      repairPrompt({ tracePath, permanentTracePath: permanent.destination, verification, attempt, verifier: options.verifier }),
      path.join(options.traceRoot, "serial_repair_logs", `${permanent.fingerprint}-attempt-${attempt}.jsonl`),
      options,
    );
    appendWorkerState(options.traceRoot, worker, { event: "repair_finished", trace: name, attempt, code: repair.code, signal: repair.signal || null });
    verification = verifyTrace(options.verifier, tracePath, logPath);
    appendWorkerState(options.traceRoot, worker, { event: "reverified", trace: name, attempt, result: summarizeVerification(verification) });
    if (verification.ok) {
      markPermanentTraceComplete(permanent.traceName);
      fs.unlinkSync(tracePath);
      appendWorkerState(options.traceRoot, worker, { event: "repaired_removed", trace: name, permanent: permanent.traceName, attempt });
      return { status: "repaired", trace: name, permanent: permanent.traceName, attempt, worker };
    }
    if (verification.outcome === "verifier_error" || !verification.boundary) {
      appendWorkerState(options.traceRoot, worker, { event: "repair_blocked", trace: name, permanent: permanent.traceName, reason: "verifier_error_or_missing_boundary" });
      return { status: "blocked", trace: name, permanent: permanent.traceName, verification, worker };
    }
    attempt += 1;
  }
  appendWorkerState(options.traceRoot, worker, { event: "repair_blocked", trace: name, permanent: permanent.traceName, reason: "max_attempts" });
  return { status: "blocked", trace: name, permanent: permanent.traceName, verification, worker };
}

function summarizeVerification(value) {
  return {
    ok: value.ok,
    outcome: value.outcome,
    exitCode: value.exitCode,
    boundary: value.boundary,
    totalActions: value.totalActions,
    verified: value.verified,
    unsupported: value.unsupported,
    unexpectedDiffs: value.unexpectedDiffs,
  };
}

async function main(argv = process.argv.slice(2)) {
  const options = parseArgs(argv);
  if (options.help) {
    console.log("usage: serial_trace_processor.js [--trace-root PATH] [--max-traces N] [--repair-agents N] [--max-repair-attempts N] [--repair-timeout-ms N] [--no-repair] [--skip-blocked]");
    return;
  }
  if (!fs.existsSync(options.verifier)) throw new Error(`verifier not found: ${options.verifier}`);
  const state = loadState(options.traceRoot);
  let skippedBlocked = 0;
  const candidates = traceFiles(options.traceRoot).filter((tracePath) => {
    const name = path.basename(tracePath);
    if (state.removed.has(name)) return false;
    if (!options.retryBlocked && state.blocked.has(name)) {
      skippedBlocked += 1;
      return false;
    }
    return true;
  });
  const traces = candidates.slice(0, options.maxTraces || undefined);
  appendState(options.traceRoot, {
    event: "started",
    trace_count: traces.length,
    skipped_blocked: skippedBlocked,
    repair_agents: options.repairAgents,
    infinite_attempts: options.maxRepairAttempts === Infinity,
    serial: false,
  });
  let nextIndex = 0;
  async function workerLoop(worker) {
    appendWorkerState(options.traceRoot, worker, { event: "worker_started" });
    while (true) {
      const tracePath = traces[nextIndex];
      nextIndex += 1;
      if (!tracePath) break;
      try {
        const result = await processTrace(tracePath, options, worker);
        console.log(JSON.stringify(result));
      } catch (error) {
        const trace = path.basename(tracePath);
        appendWorkerState(options.traceRoot, worker, { event: "worker_error", trace, error: error.stack || String(error) });
        console.error(JSON.stringify({ status: "worker_error", trace, worker, error: error.message }));
      }
    }
    appendWorkerState(options.traceRoot, worker, { event: "worker_stopped" });
  }
  const workerCount = Math.min(options.repairAgents, Math.max(1, traces.length));
  await Promise.all(Array.from({ length: workerCount }, (_, index) => workerLoop(index + 1)));
  appendState(options.traceRoot, { event: "stopped" });
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exitCode = 1;
  });
}

module.exports = {
  parseArgs,
  parseVerifierOutput,
  permanentizeFailure,
  processTrace,
  summarizeVerification,
  traceFiles,
  traceFingerprint,
};
