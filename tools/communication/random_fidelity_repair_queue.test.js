#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const {
  claimTask,
  finishTask,
  readTasks,
  recheckDisposition,
  reopenTask,
  satisfyTask,
  summarize,
} = require("./random_fidelity_repair_queue");

const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-random-repair-queue-"));
try {
  for (const [fingerprint, occurrences] of [["one", 1], ["two", 2]]) {
    const directory = path.join(root, fingerprint);
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(
      path.join(directory, "task.json"),
      `${JSON.stringify({
        schema: 1,
        fingerprint,
        status: "queued",
        first_seen_at: fingerprint === "one" ? "2026-01-01T00:00:00Z" : "2026-01-02T00:00:00Z",
        occurrences: Array.from({ length: occurrences }, (_, index) => ({ index })),
      })}\n`,
    );
  }

  let entries = readTasks(root);
  const claimed = claimTask(entries, "agent-a", null, "2026-01-03T00:00:00Z");
  assert.strictEqual(claimed.fingerprint, "two");
  assert.strictEqual(claimed.status, "in_progress");
  assert.strictEqual(claimed.repair.attempts, 1);

  entries = readTasks(root);
  assert.deepStrictEqual(summarize(entries), {
    total: 2,
    statuses: { queued: 1, in_progress: 1 },
    queued_occurrences: 1,
  });
  assert.throws(
    () => finishTask(entries, "two", "wrong-agent", "resolved"),
    /claimed by agent-a/,
  );
  const resolved = finishTask(
    entries,
    "two",
    "agent-a",
    "resolved",
    "focused parity passed",
    "2026-01-04T00:00:00Z",
  );
  assert.strictEqual(resolved.status, "resolved");
  assert.strictEqual(resolved.repair.note, "focused parity passed");

  entries = readTasks(root);
  const first = claimTask(entries, "agent-b", "one", "2026-01-05T00:00:00Z");
  assert.strictEqual(first.fingerprint, "one");
  const released = finishTask(entries, "one", "agent-b", "queued", "blocked");
  assert.strictEqual(released.status, "queued");
  const orphanPath = path.join(root, "one", "task.json");
  const orphan = JSON.parse(fs.readFileSync(orphanPath, "utf8"));
  delete orphan.repair;
  orphan.status = "in_progress";
  fs.writeFileSync(orphanPath, `${JSON.stringify(orphan)}\n`);
  const reclaimed = claimTask(readTasks(root), "agent-c", "one");
  assert.strictEqual(reclaimed.repair.worker, "agent-c");
  assert.deepStrictEqual(recheckDisposition("old", { fingerprint: "old" }), {
    status: "queued",
    note: "repair recheck still fails with old",
  });
  assert.deepStrictEqual(recheckDisposition("old", { fingerprint: "new" }), {
    status: "resolved",
    note: "repair recheck advanced to new",
  });
  assert.deepStrictEqual(recheckDisposition("old", { fingerprint: null }), {
    status: "resolved",
    note: "repair recheck reached strict parity",
  });
  const reopened = reopenTask(readTasks(root), "two", "new boundary was infrastructure-only");
  assert.strictEqual(reopened.status, "queued");
  assert.strictEqual(reopened.repair.reopen_note, "new boundary was infrastructure-only");
  const satisfied = satisfyTask(
    readTasks(root),
    "two",
    "authoritative corpus replay verified the retained prefix",
  );
  assert.strictEqual(satisfied.status, "resolved");
  assert.match(satisfied.repair.note, /authoritative corpus replay/);
  const inProgressPath = path.join(root, "one", "task.json");
  const inProgress = JSON.parse(fs.readFileSync(inProgressPath, "utf8"));
  inProgress.status = "in_progress";
  inProgress.repair = { worker: "stale-worker" };
  fs.writeFileSync(inProgressPath, `${JSON.stringify(inProgress)}\n`);
  const cancelled = satisfyTask(
    readTasks(root),
    "one",
    "authoritative corpus replay superseded the active claim",
  );
  assert.strictEqual(cancelled.status, "resolved");
  assert.strictEqual(cancelled.repair.worker, "stale-worker");
  assert.match(cancelled.repair.note, /superseded/);
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}

console.log("random_fidelity_repair_queue tests passed");
