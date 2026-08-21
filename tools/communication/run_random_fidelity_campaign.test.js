#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const {
  firstUncollectedPolicySeed,
  isInfrastructureFailure,
  retryDelayForFailures,
} = require("./run_random_fidelity_campaign");

assert.strictEqual(retryDelayForFailures(1), 5000);
assert.strictEqual(retryDelayForFailures(2), 10000);
assert.strictEqual(retryDelayForFailures(3), 20000);
assert.strictEqual(retryDelayForFailures(4), 30000);
assert.strictEqual(retryDelayForFailures(20), 30000);
assert.strictEqual(
  isInfrastructureFailure({
    elapsed_ms: 100,
    stderr: "Error: connect ECONNREFUSED 127.0.0.1:1234",
  }),
  true,
);
assert.strictEqual(
  isInfrastructureFailure({
    elapsed_ms: 60_000,
    stderr: "Error: connect ECONNREFUSED 127.0.0.1:1234",
  }),
  false,
);
assert.strictEqual(
  isInfrastructureFailure({
    elapsed_ms: 60_000,
    stderr: "Error: a command is already in flight",
  }),
  true,
);
assert.strictEqual(
  isInfrastructureFailure({
    elapsed_ms: 60_000,
    stderr: "Error: bridge is not ready for a command",
  }),
  true,
);
assert.strictEqual(
  isInfrastructureFailure({
    elapsed_ms: 60_000,
    stderr: "Error: START_VERIFY did not become available at the main menu",
  }),
  true,
);

const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), "sts-campaign-resume-"));
try {
  const tracesDir = path.join(outputDir, "traces");
  fs.mkdirSync(tracesDir);
  fs.writeFileSync(path.join(tracesDir, "FIDL00127-p127-old.jsonl"), "");
  fs.writeFileSync(path.join(tracesDir, "FIDL00125-p125-old.jsonl"), "");
  fs.writeFileSync(path.join(tracesDir, "OTHER00999-p999-old.jsonl"), "");
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 1), 128);
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 200), 200);
  fs.writeFileSync(
    path.join(outputDir, "skipped_policy_seeds.jsonl"),
    [
      JSON.stringify({ seed_prefix: "FIDL", policy_seed: 130 }),
      "{malformed",
      JSON.stringify({ seed_prefix: "OTHER", policy_seed: 999 }),
      "",
    ].join("\n"),
  );
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 1), 131);
} finally {
  fs.rmSync(outputDir, { recursive: true, force: true });
}

console.log("random fidelity campaign tests passed");
