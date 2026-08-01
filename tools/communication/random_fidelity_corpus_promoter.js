#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const {
  promoteDistinctFailure,
} = require("./random_fidelity_collector");

const root = path.resolve(__dirname, "..", "..");
const outputDir = path.resolve(
  process.env.STS_RANDOM_OUTPUT_DIR ||
    path.join(root, "simulator", "target", "random-fidelity"),
);
const tasksDir = path.join(outputDir, "repair_tasks");
const pollMs = Number.parseInt(process.env.STS_RANDOM_PROMOTE_POLL_MS || "1000", 10);

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

function promotePending(tasks = readTasks()) {
  let added = 0;
  let existing = 0;
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
  }
  return { added, existing, expectations_updated: 0, skipped };
}

async function main() {
  console.error(`random permanent-corpus promoter ready: ${tasksDir}`);
  for (;;) {
    const summary = promotePending();
    if (summary.added > 0) {
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
  promotionCandidate,
  promotePending,
  readTasks,
};
