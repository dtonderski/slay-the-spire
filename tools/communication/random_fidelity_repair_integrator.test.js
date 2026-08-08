#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const {
  focusedDisposition,
  gateHasNoNewFailures,
  nextCandidate,
  recoverGatingCandidates,
  uvCommand,
  verifierBinaryPath,
} = require("./random_fidelity_repair_integrator");

assert.deepStrictEqual(
  focusedDisposition("old", { fingerprint: null, status: "verified" }),
  { accepted: true, note: "focused replay reached strict parity" },
);
assert.deepStrictEqual(
  focusedDisposition("old", { fingerprint: "new", status: "divergence" }),
  { accepted: true, note: "focused replay advanced to new" },
);
assert.deepStrictEqual(
  focusedDisposition("old", { fingerprint: "old", status: "divergence" }),
  { accepted: false, note: "focused replay still fails with old" },
);

assert.strictEqual(
  nextCandidate([
    { status: "gating", fingerprint: "a" },
    { status: "queued", fingerprint: "b", created_at: "2026-01-02T00:00:00Z" },
    { status: "queued", fingerprint: "c", created_at: "2026-01-01T00:00:00Z" },
  ]).fingerprint,
  "c",
);
assert.strictEqual(
  gateHasNoNewFailures(["old-a.jsonl", "old-b.jsonl"], ["old-b.jsonl"]),
  true,
);
assert.strictEqual(
  gateHasNoNewFailures(["old-a.jsonl"], ["old-a.jsonl", "new.jsonl"]),
  false,
);
assert.strictEqual(
  verifierBinaryPath("/tmp/integration-target"),
  process.platform === "win32"
    ? "\\tmp\\integration-target\\release\\sts_verify.exe"
    : "/tmp/integration-target/release/sts_verify",
);
assert.strictEqual(
  uvCommand({ STS_RANDOM_UV_BIN: "/opt/tools/uv" }),
  "/opt/tools/uv",
);

const recoveryRoot = fs.mkdtempSync(path.join(os.tmpdir(), "random-integrator-recovery-"));
const recoveryDir = path.join(recoveryRoot, "repair_candidates", "abc");
fs.mkdirSync(recoveryDir, { recursive: true });
fs.writeFileSync(
  path.join(recoveryDir, "candidate.json"),
  `${JSON.stringify({
    fingerprint: "abc",
    status: "gating",
    created_at: "2026-01-01T00:00:00Z",
  })}\n`,
);
assert.deepStrictEqual(recoverGatingCandidates(recoveryRoot), ["abc"]);
const recovered = JSON.parse(
  fs.readFileSync(path.join(recoveryDir, "candidate.json"), "utf8"),
);
assert.strictEqual(recovered.status, "queued");
assert.strictEqual(
  recovered.recovery_note,
  "integrator restarted while candidate was gating",
);
fs.rmSync(recoveryRoot, { recursive: true, force: true });

console.log("random fidelity repair integrator tests passed");
