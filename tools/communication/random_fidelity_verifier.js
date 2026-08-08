#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const {
  expectedFailureBoundary,
  fingerprint,
  isPromotableFailure,
  promoteDistinctFailure,
  verifyTrace,
} = require("./random_fidelity_collector");

const root = path.resolve(__dirname, "..", "..");
const outputDir = path.resolve(
  process.env.STS_RANDOM_OUTPUT_DIR ||
    path.join(root, "simulator", "target", "random-fidelity"),
);
const queuePath = path.join(outputDir, "verification_queue.jsonl");
const resultsPath = path.join(outputDir, "verification_results.jsonl");
const tasksDir = path.join(outputDir, "repair_tasks");
const taskLocksDir = path.join(outputDir, "repair_task_locks");
const verifierPath = path.resolve(
  process.env.STS_VERIFY_BIN ||
    path.join(root, "simulator", "target", "release", "sts_verify"),
);
const pollMs = Number.parseInt(process.env.STS_RANDOM_VERIFY_POLL_MS || "250", 10);
const workerCount = Number.parseInt(process.env.STS_RANDOM_VERIFY_WORKERS || "1", 10);
const workerIndex = Number.parseInt(process.env.STS_RANDOM_VERIFY_WORKER_INDEX || "0", 10);

if (!Number.isInteger(workerCount) || workerCount < 1) {
  throw new Error("STS_RANDOM_VERIFY_WORKERS must be a positive integer");
}
if (!Number.isInteger(workerIndex) || workerIndex < 0 || workerIndex >= workerCount) {
  throw new Error("STS_RANDOM_VERIFY_WORKER_INDEX must be in [0, worker count)");
}

function readJsonl(filePath) {
  try {
    return fs
      .readFileSync(filePath, "utf8")
      .split(/\r?\n/)
      .filter(Boolean)
      .flatMap((line) => {
        try {
          return [JSON.parse(line)];
        } catch {
          return [];
        }
      });
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

function assignedToWorker(queueIndex, count, index) {
  return queueIndex % count === index;
}

function writeJsonAtomic(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const temporary = `${filePath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
  fs.renameSync(temporary, filePath);
}

function withTaskLock(id, callback) {
  fs.mkdirSync(taskLocksDir, { recursive: true });
  const lockPath = path.join(taskLocksDir, id);
  for (;;) {
    try {
      fs.mkdirSync(lockPath);
      break;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 25);
    }
  }
  try {
    return callback();
  } finally {
    fs.rmdirSync(lockPath);
  }
}

function updateRepairTask(entry, verification, minimizedTrace) {
  const id = fingerprint(verification);
  return withTaskLock(id, () => {
    const taskPath = path.join(tasksDir, id, "task.json");
    let task = null;
    try {
      task = JSON.parse(fs.readFileSync(taskPath, "utf8"));
    } catch {}
    const occurrence = {
      recorded_at: new Date().toISOString(),
      trace: entry.trace,
      minimized_trace: minimizedTrace,
      game_seed: entry.game_seed,
      policy_seed: entry.policy_seed,
      actions: entry.actions,
    };
    task = {
      ...(task || {}),
      schema: 1,
      fingerprint: id,
      status: task?.status || "queued",
      first_seen_at: task?.first_seen_at || occurrence.recorded_at,
      updated_at: occurrence.recorded_at,
      boundary: expectedFailureBoundary(verification),
      boundary_reason: verification.boundaryReason,
      first_diff: verification.firstDiff,
      diff_lines: verification.diffLines,
      occurrences: [...(task?.occurrences || []), occurrence],
    };
    writeJsonAtomic(taskPath, task);
    return task;
  });
}

function minimizeTrace(entry, id) {
  const directory = path.join(outputDir, "minimized");
  fs.mkdirSync(directory, { recursive: true });
  const destination = path.join(
    directory,
    `${entry.game_seed}-p${entry.policy_seed}-${id}.jsonl`,
  );
  if (fs.existsSync(destination)) return destination;
  const child = spawnSync(
    verifierPath,
    ["minimize", "-o", destination, entry.trace],
    { cwd: root, encoding: "utf8", windowsHide: true },
  );
  return child.status === 0 && fs.existsSync(destination) ? destination : null;
}

function isVerifierInfrastructureError(error) {
  return /spawnSync .+ (?:EACCES|ENOENT|EPERM)\b/.test(String(error?.message || error));
}

function verificationDisposition(verification, verifierError, acceptVerifiedPrefix = false) {
  if (verifierError) return { failed: false, status: "verifier_error" };
  const hasDivergence =
    verification.unexpectedDiffs > 0 ||
    Boolean(
      verification.boundaryCategory &&
        verification.boundaryCategory !== "none",
    );
  if (hasDivergence) return { failed: true, status: "divergence" };
  if (verification.exitCode === 0) return { failed: false, status: "verified" };
  const cleanPrefix =
    Number(verification.duplicateDispositions || 0) === 0 &&
    Number(verification.unresolvedTransientAssertions || 0) === 0;
  if (acceptVerifiedPrefix && cleanPrefix) {
    return { failed: false, status: "verified_prefix" };
  }
  return { failed: false, status: "trace_integrity_error" };
}

function verifyEntry(entry, { acceptVerifiedPrefix = false } = {}) {
  const started = Date.now();
  let verification;
  let verifierError = null;
  try {
    verification = verifyTrace(verifierPath, entry.trace);
  } catch (error) {
    if (isVerifierInfrastructureError(error)) throw error;
    verifierError = error;
    verification = {
      exitCode: null,
      unexpectedDiffs: 0,
      boundaryPath: "$.verifier",
      boundaryCategory: "verifier_crash",
      boundaryReason: error.message,
      firstDiff: null,
      diffLines: [],
    };
  }
  const disposition = verificationDisposition(
    verification,
    verifierError,
    acceptVerifiedPrefix,
  );
  const { failed } = disposition;
  let task = null;
  let minimizedTrace = null;
  let promotion = null;
  if (failed) {
    const id = fingerprint(verification);
    if (!verifierError) minimizedTrace = minimizeTrace(entry, id);
    if (minimizedTrace && isPromotableFailure(verification)) {
      const boundary = expectedFailureBoundary(verification);
      promotion = promoteDistinctFailure({
        minimizedPath: minimizedTrace,
        fingerprint: id,
        boundaryPath: boundary.path,
        boundaryCategory: boundary.category,
      });
    }
    task = updateRepairTask(entry, verification, minimizedTrace);
  }
  return {
    verified_at: new Date().toISOString(),
    trace: entry.trace,
    game_seed: entry.game_seed,
    policy_seed: entry.policy_seed,
    actions: entry.actions,
    status: disposition.status,
    elapsed_ms: Date.now() - started,
    boundary: failed ? expectedFailureBoundary(verification) : null,
    boundary_reason: verification.boundaryReason,
    first_diff: verification.firstDiff,
    fingerprint: task?.fingerprint || null,
    minimized_trace: minimizedTrace,
    permanent_trace: promotion?.trace || null,
    newly_promoted: promotion?.added || false,
  };
}

async function main() {
  fs.mkdirSync(outputDir, { recursive: true });
  fs.mkdirSync(tasksDir, { recursive: true });
  console.error(`random verifier worker ${workerIndex + 1}/${workerCount} ready`);
  const handled = new Set(
    readJsonl(resultsPath)
      .filter((entry) => entry.status !== "verifier_error")
      .map((entry) => entry.trace)
      .filter(Boolean),
  );
  for (;;) {
    const next = readJsonl(queuePath).find(
      (entry, index) =>
        assignedToWorker(index, workerCount, workerIndex) &&
        entry.trace &&
        !handled.has(entry.trace),
    );
    if (!next) {
      await new Promise((resolve) => setTimeout(resolve, pollMs));
      continue;
    }
    try {
      const result = verifyEntry(next);
      fs.appendFileSync(resultsPath, `${JSON.stringify(result)}\n`);
      handled.add(next.trace);
      console.log(JSON.stringify({ ...result, verifier_worker: workerIndex }));
    } catch (error) {
      console.error(error.stack || error);
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exit(1);
  });
}

module.exports = {
  assignedToWorker,
  isVerifierInfrastructureError,
  readJsonl,
  verificationDisposition,
  verifierPath,
  verifyEntry,
  writeJsonAtomic,
};
