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
  const recordTrace = (prefix, policySeed, metadataPolicySeed = policySeed) => {
    const gameSeed = `${prefix}${String(policySeed).padStart(5, "0")}`;
    fs.writeFileSync(
      path.join(tracesDir, `${gameSeed}-p${policySeed}-old.jsonl`),
      `${JSON.stringify({
        type: "metadata",
        collection: { policy_seed: metadataPolicySeed, game_seed: gameSeed },
      })}\n`,
    );
  };
  recordTrace("FIDL", 1);
  recordTrace("FIDL", 3);
  recordTrace("OTHER", 1);
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 1), 2);
  recordTrace("FIDL", 2);
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 1), 4);
  recordTrace("FIDL", 4, 999);
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 1), 4);
  fs.writeFileSync(path.join(tracesDir, "FIDL00005-p5-malformed.jsonl"), "{malformed\n");
  fs.writeFileSync(
    path.join(outputDir, "skipped_policy_seeds.jsonl"),
    `${JSON.stringify({ seed_prefix: "FIDL", policy_seed: 4 })}\n`,
  );
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 1), 4);
  assert.strictEqual(firstUncollectedPolicySeed(outputDir, "FIDL", 200), 200);
} finally {
  fs.rmSync(outputDir, { recursive: true, force: true });
}

console.log("random fidelity campaign tests passed");
