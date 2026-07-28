#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const {
  acquireDirectoryLock,
  promoteDistinctFailure,
} = require("./random_fidelity_collector");

const root = path.resolve(__dirname, "..", "..");
const outputDir = path.resolve(
  process.env.STS_RANDOM_OUTPUT_DIR ||
    path.join(root, "simulator", "target", "random-fidelity"),
);
const tasksDir = path.join(outputDir, "repair_tasks");
const pollMs = Number.parseInt(process.env.STS_RANDOM_PROMOTE_POLL_MS || "1000", 10);
const corpusRoot = path.join(root, "simulator", "verification", "corpus");
const manifestPath = path.join(corpusRoot, "permanent_traces.json");
const manifestLockPath = path.join(corpusRoot, ".permanent-traces.lock");

if (!Number.isInteger(pollMs) || pollMs < 1) {
  throw new Error("STS_RANDOM_PROMOTE_POLL_MS must be a positive integer");
}

function readTasks(directory = tasksDir) {
  try {
    return fs.readdirSync(directory).flatMap((name) => {
      const taskPath = path.join(directory, name, "task.json");
      try {
        return [JSON.parse(fs.readFileSync(taskPath, "utf8"))];
      } catch (error) {
        if (error.code === "ENOENT" || error instanceof SyntaxError) return [];
        throw error;
      }
    });
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

function promotionCandidate(task, traceExists = fs.existsSync) {
  const nonDivergenceCategories = new Set([
    "trace_integrity_error",
    "verifier_crash",
  ]);
  if (
    !task?.boundary?.category ||
    nonDivergenceCategories.has(task.boundary.category) ||
    !task.fingerprint
  ) {
    return null;
  }
  const minimizedPath = (task.occurrences || [])
    .map((occurrence) => occurrence.minimized_trace)
    .find((candidate) => candidate && traceExists(candidate));
  if (!minimizedPath) return null;
  return {
    minimizedPath,
    fingerprint: task.fingerprint,
    boundaryPath: task.boundary.path,
    boundaryCategory: task.boundary.category,
  };
}

function expectationForTask(task, traceText) {
  if (task.status !== "resolved") {
    return {
      kind: "expected_boundary",
      boundary: {
        path: task.boundary.path,
        category: task.boundary.category,
      },
    };
  }
  const lastActionStep = traceText
    .split(/\r?\n/)
    .filter(Boolean)
    .flatMap((line) => {
      try {
        const record = JSON.parse(line);
        return record.type === "action" && Number.isInteger(record.step)
          ? [record.step]
          : [];
      } catch {
        return [];
      }
    })
    .reduce((maximum, step) => Math.max(maximum, step), 0);
  if (lastActionStep < 1) {
    throw new Error(`resolved trace for ${task.fingerprint} has no action step`);
  }
  return {
    kind: "retained_prefix",
    endpoint: {
      action_step: lastActionStep,
      label: `resolved random-fidelity regression ${task.fingerprint}`,
    },
  };
}

function withManifestLock(callback) {
  acquireDirectoryLock(manifestLockPath);
  try {
    return callback();
  } finally {
    fs.rmdirSync(manifestLockPath);
  }
}

function reconcileExpectation(task, candidate) {
  const traceName = `random-fidelity-${task.fingerprint}.jsonl`;
  const permanentTracePath = path.join(corpusRoot, "permanent_traces", traceName);
  if (!fs.existsSync(permanentTracePath)) return false;
  const expectation = expectationForTask(
    task,
    fs.readFileSync(permanentTracePath, "utf8"),
  );
  return withManifestLock(() => {
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    const entry = manifest.entries.find((value) => value.trace === traceName);
    if (!entry) return false;
    if (JSON.stringify(entry.expectation) === JSON.stringify(expectation)) return false;
    entry.expectation = expectation;
    const temporary = `${manifestPath}.tmp-${process.pid}`;
    fs.writeFileSync(temporary, `${JSON.stringify(manifest, null, 2)}\n`);
    fs.renameSync(temporary, manifestPath);
    return true;
  });
}

function promotePending(tasks = readTasks()) {
  let added = 0;
  let existing = 0;
  let expectationsUpdated = 0;
  let skipped = 0;
  for (const task of tasks) {
    const candidate = promotionCandidate(task);
    if (!candidate) {
      skipped += 1;
      continue;
    }
    const result = promoteDistinctFailure(candidate);
    if (result?.added) added += 1;
    else existing += 1;
    if (reconcileExpectation(task, candidate)) expectationsUpdated += 1;
  }
  return { added, existing, expectations_updated: expectationsUpdated, skipped };
}

async function main() {
  console.error(`random permanent-corpus promoter ready: ${tasksDir}`);
  for (;;) {
    const summary = promotePending();
    if (summary.added > 0 || summary.expectations_updated > 0) {
      console.log(JSON.stringify({
        promoted_at: new Date().toISOString(),
        ...summary,
      }));
    }
    await new Promise((resolve) => setTimeout(resolve, pollMs));
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exit(1);
  });
}

module.exports = {
  expectationForTask,
  promotionCandidate,
  promotePending,
  reconcileExpectation,
  readTasks,
};
