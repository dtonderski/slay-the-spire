#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

function writeJsonAtomic(filePath, value) {
  const temporary = `${filePath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
  fs.renameSync(temporary, filePath);
}

function withTaskLock(tasksDir, fingerprint, callback) {
  const locksDir = path.join(path.dirname(tasksDir), "repair_task_locks");
  const lockPath = path.join(locksDir, fingerprint);
  fs.mkdirSync(locksDir, { recursive: true });
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

function readTasks(tasksDir) {
  try {
    return fs
      .readdirSync(tasksDir, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .flatMap((entry) => {
        const taskPath = path.join(tasksDir, entry.name, "task.json");
        try {
          return [{ taskPath, task: JSON.parse(fs.readFileSync(taskPath, "utf8")) }];
        } catch {
          return [];
        }
      });
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

function rankTasks(entries) {
  return [...entries].sort(
    (left, right) =>
      (right.task.occurrences?.length || 0) - (left.task.occurrences?.length || 0) ||
      String(left.task.first_seen_at).localeCompare(String(right.task.first_seen_at)),
  );
}

function claimTask(entries, worker, requestedFingerprint = null, now = new Date().toISOString()) {
  const candidate = rankTasks(entries).find(
    ({ task }) =>
      (task.status === "queued" ||
        (task.status === "in_progress" && !task.repair?.worker)) &&
      (!requestedFingerprint || task.fingerprint === requestedFingerprint),
  );
  if (!candidate) return null;
  candidate.task = {
    ...candidate.task,
    status: "in_progress",
    updated_at: now,
    repair: {
      worker,
      claimed_at: now,
      attempts: Number(candidate.task.repair?.attempts || 0) + 1,
    },
  };
  writeJsonAtomic(candidate.taskPath, candidate.task);
  return candidate.task;
}

function finishTask(entries, fingerprint, worker, status, note, now = new Date().toISOString()) {
  const candidate = entries.find(({ task }) => task.fingerprint === fingerprint);
  if (!candidate) throw new Error(`unknown repair fingerprint ${fingerprint}`);
  if (candidate.task.status !== "in_progress") {
    throw new Error(`repair task ${fingerprint} is ${candidate.task.status}, not in_progress`);
  }
  if (candidate.task.repair?.worker !== worker) {
    throw new Error(`repair task ${fingerprint} is claimed by ${candidate.task.repair?.worker}`);
  }
  candidate.task = {
    ...candidate.task,
    status,
    updated_at: now,
    repair: {
      ...candidate.task.repair,
      finished_at: now,
      note: note || null,
    },
  };
  writeJsonAtomic(candidate.taskPath, candidate.task);
  return candidate.task;
}

function reopenTask(entries, fingerprint, note, now = new Date().toISOString()) {
  const candidate = entries.find(({ task }) => task.fingerprint === fingerprint);
  if (!candidate) throw new Error(`unknown repair fingerprint ${fingerprint}`);
  candidate.task = {
    ...candidate.task,
    status: "queued",
    updated_at: now,
    repair: {
      ...(candidate.task.repair || {}),
      reopened_at: now,
      reopen_note: note || null,
    },
  };
  writeJsonAtomic(candidate.taskPath, candidate.task);
  return candidate.task;
}

function satisfyTask(entries, fingerprint, note, now = new Date().toISOString()) {
  const candidate = entries.find(({ task }) => task.fingerprint === fingerprint);
  if (!candidate || candidate.task.status === "resolved") return candidate?.task || null;
  if (!new Set(["queued", "in_progress"]).has(candidate.task.status)) {
    throw new Error(
      `cannot satisfy repair task ${fingerprint} while it is ${candidate.task.status}`,
    );
  }
  candidate.task = {
    ...candidate.task,
    status: "resolved",
    updated_at: now,
    repair: {
      ...(candidate.task.repair || {}),
      finished_at: now,
      note: note || "authoritative replay no longer reaches this boundary",
    },
  };
  writeJsonAtomic(candidate.taskPath, candidate.task);
  return candidate.task;
}

function satisfyTaskByFingerprint(tasksDir, fingerprint, note) {
  return withTaskLock(tasksDir, fingerprint, () =>
    satisfyTask(readTasks(tasksDir), fingerprint, note));
}

function summarize(entries) {
  const statuses = {};
  for (const { task } of entries) {
    statuses[task.status] = (statuses[task.status] || 0) + 1;
  }
  return {
    total: entries.length,
    statuses,
    queued_occurrences: entries
      .filter(({ task }) => task.status === "queued")
      .reduce((sum, { task }) => sum + (task.occurrences?.length || 0), 0),
  };
}

function recheckDisposition(fingerprint, result) {
  if (result.fingerprint === fingerprint) {
    return {
      status: "queued",
      note: `repair recheck still fails with ${fingerprint}`,
    };
  }
  return {
    status: "resolved",
    note: result.fingerprint
      ? `repair recheck advanced to ${result.fingerprint}`
      : "repair recheck reached strict parity",
  };
}

function claimNextTask(tasksDir, worker, fingerprint = null) {
  const candidate = rankTasks(readTasks(tasksDir)).find(
    ({ task }) =>
      (task.status === "queued" ||
        (task.status === "in_progress" && !task.repair?.worker)) &&
      (!fingerprint || task.fingerprint === fingerprint),
  );
  return candidate
    ? withTaskLock(tasksDir, candidate.task.fingerprint, () =>
      claimTask(readTasks(tasksDir), worker, candidate.task.fingerprint))
    : null;
}

function main() {
  const outputDir = path.resolve(
    process.env.STS_RANDOM_OUTPUT_DIR ||
      path.join(__dirname, "..", "..", "simulator", "target", "random-fidelity"),
  );
  const tasksDir = path.join(outputDir, "repair_tasks");
  const [command = "status", ...args] = process.argv.slice(2);
  const entries = readTasks(tasksDir);
  let result;
  if (command === "status") {
    result = summarize(entries);
  } else if (command === "claim") {
    const [worker, fingerprint] = args;
    if (!worker) throw new Error("usage: random_fidelity_repair_queue.js claim WORKER [FINGERPRINT]");
    result = claimNextTask(tasksDir, worker, fingerprint);
    if (!result) process.exitCode = 2;
  } else if (command === "claim-ready") {
    const [worker] = args;
    if (!worker) throw new Error("usage: random_fidelity_repair_queue.js claim-ready WORKER");
    const { verifyEntry } = require("./random_fidelity_verifier");
    const skipped = [];
    result = null;
    for (let attempt = 0; attempt < 100; attempt += 1) {
      const claimed = claimNextTask(tasksDir, worker);
      if (!claimed) break;
      const occurrence = claimed.occurrences?.[0];
      if (!occurrence?.trace) {
        withTaskLock(tasksDir, claimed.fingerprint, () =>
          finishTask(
            readTasks(tasksDir),
            claimed.fingerprint,
            worker,
            "resolved",
            "claim preflight found no replayable trace",
          ));
        skipped.push({ fingerprint: claimed.fingerprint, status: "unreplayable" });
        continue;
      }
      const verification = verifyEntry(occurrence);
      const disposition = recheckDisposition(claimed.fingerprint, verification);
      if (disposition.status === "queued") {
        result = { task: claimed, verification, skipped };
        break;
      }
      withTaskLock(tasksDir, claimed.fingerprint, () =>
        finishTask(
          readTasks(tasksDir),
          claimed.fingerprint,
          worker,
          "resolved",
          `claim preflight: ${disposition.note}`,
        ));
      skipped.push({
        fingerprint: claimed.fingerprint,
        status: verification.status,
        advanced_to: verification.fingerprint,
      });
    }
    if (!result) {
      result = { task: null, verification: null, skipped };
      process.exitCode = 2;
    }
  } else if (command === "resolve" || command === "release") {
    const [fingerprint, worker, ...noteParts] = args;
    if (!fingerprint || !worker) {
      throw new Error(`usage: random_fidelity_repair_queue.js ${command} FINGERPRINT WORKER [NOTE]`);
    }
    result = withTaskLock(
      tasksDir,
      fingerprint,
      () => finishTask(
        readTasks(tasksDir),
        fingerprint,
        worker,
        command === "resolve" ? "resolved" : "queued",
        noteParts.join(" "),
      ),
    );
  } else if (command === "recheck") {
    const [fingerprint, worker] = args;
    if (!fingerprint || !worker) {
      throw new Error("usage: random_fidelity_repair_queue.js recheck FINGERPRINT WORKER");
    }
    const candidate = entries.find(({ task }) => task.fingerprint === fingerprint);
    if (!candidate) throw new Error(`unknown repair fingerprint ${fingerprint}`);
    if (candidate.task.status !== "in_progress" || candidate.task.repair?.worker !== worker) {
      throw new Error(`repair task ${fingerprint} is not claimed by ${worker}`);
    }
    const occurrence = candidate.task.occurrences?.[0];
    if (!occurrence?.trace) throw new Error(`repair task ${fingerprint} has no replayable trace`);
    const { verifyEntry } = require("./random_fidelity_verifier");
    const verification = verifyEntry(occurrence);
    const disposition = recheckDisposition(fingerprint, verification);
    const completed = withTaskLock(
      tasksDir,
      fingerprint,
      () => finishTask(
        readTasks(tasksDir),
        fingerprint,
        worker,
        disposition.status,
        disposition.note,
      ),
    );
    const recheck = { rechecked_at: new Date().toISOString(), fingerprint, worker, verification };
    fs.appendFileSync(
      path.join(outputDir, "repair_rechecks.jsonl"),
      `${JSON.stringify(recheck)}\n`,
    );
    result = { task: completed, verification };
  } else if (command === "reopen") {
    const [fingerprint, ...noteParts] = args;
    if (!fingerprint) {
      throw new Error("usage: random_fidelity_repair_queue.js reopen FINGERPRINT [NOTE]");
    }
    result = withTaskLock(
      tasksDir,
      fingerprint,
      () => reopenTask(readTasks(tasksDir), fingerprint, noteParts.join(" ")),
    );
  } else {
    throw new Error(`unknown command ${command}`);
  }
  console.log(JSON.stringify(result, null, 2));
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

module.exports = {
  claimNextTask,
  claimTask,
  finishTask,
  rankTasks,
  readTasks,
  recheckDisposition,
  reopenTask,
  satisfyTask,
  satisfyTaskByFingerprint,
  summarize,
  withTaskLock,
};
