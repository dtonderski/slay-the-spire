#!/usr/bin/env node

const assert = require("assert");
const path = require("path");
const {
  codexArgs,
  claimedFingerprintsForWorker,
  parseClaim,
  repairCodexCommand,
  repairProcessEnvironment,
  repairPrompt,
  retryDelayMs,
  workerIdentity,
} = require("./random_fidelity_repair_worker");

assert.strictEqual(workerIdentity(2), "overnight-luna-3");
assert.strictEqual(retryDelayMs("2500"), 2500);
assert.throws(() => retryDelayMs("0"), /positive integer/);
assert.strictEqual(
  repairCodexCommand({
    STS_RANDOM_REPAIR_CODEX: "/preferred/codex",
    STS_RANDOM_REPAIR_CODEX_BIN: "/fallback/codex",
  }),
  "/preferred/codex",
);
assert.strictEqual(
  repairCodexCommand({ STS_RANDOM_REPAIR_CODEX_BIN: "/fallback/codex" }),
  "/fallback/codex",
);
assert.strictEqual(
  repairProcessEnvironment({ PATH: "/usr/bin", KEEP: "yes" }, "/opt/node/bin/node").PATH,
  `/opt/node/bin${path.delimiter}/usr/bin`,
);
assert.strictEqual(
  repairProcessEnvironment({ PATH: "/usr/bin", KEEP: "yes" }, "/opt/node/bin/node").KEEP,
  "yes",
);

const args = codexArgs({
  cwd: "/tmp/worktree",
  model: "gpt-5.6-luna",
  effort: "xhigh",
});
assert.deepStrictEqual(args.slice(0, 3), ["exec", "--json", "--ephemeral"]);
assert.ok(args.includes("gpt-5.6-luna"));
assert.ok(args.includes('model_reasoning_effort="xhigh"'));
assert.ok(args.includes('service_tier="priority"'));
assert.strictEqual(args.at(-1), "-");

assert.strictEqual(parseClaim("", 2), null);
assert.strictEqual(
  parseClaim(JSON.stringify({ task: null, skipped: [] }), 2),
  null,
);
const claim = parseClaim(JSON.stringify({
  task: {
    fingerprint: "abc123",
    occurrences: [{ trace: "/tmp/full.jsonl" }],
  },
}), 0);
assert.strictEqual(claim.fingerprint, "abc123");
assert.match(repairPrompt(claim, "overnight-luna-3"), /abc123/);
assert.match(repairPrompt(claim, "overnight-luna-3"), /do not spawn\s+subagents/i);
assert.match(repairPrompt(claim, "overnight-luna-3"), /isolated candidate workspace/i);
assert.doesNotMatch(repairPrompt(claim, "overnight-luna-3"), /repair_queue\.js recheck/);

const fs = require("fs");
const os = require("os");
const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), "sts-repair-recovery-"));
try {
  for (const [fingerprint, status, worker] of [
    ["owned", "in_progress", "overnight-luna-3"],
    ["other", "in_progress", "overnight-luna-4"],
    ["done", "resolved", "overnight-luna-3"],
  ]) {
    const directory = path.join(outputDir, "repair_tasks", fingerprint);
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(
      path.join(directory, "task.json"),
      `${JSON.stringify({ fingerprint, status, repair: { worker } })}\n`,
    );
  }
  assert.deepStrictEqual(
    claimedFingerprintsForWorker(outputDir, "overnight-luna-3"),
    ["owned"],
  );
} finally {
  fs.rmSync(outputDir, { recursive: true, force: true });
}

console.log("random fidelity repair worker tests passed");
