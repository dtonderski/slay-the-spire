#!/usr/bin/env node

const assert = require("assert");
const {
  parseArgs,
  parseVerifierOutput,
  summarizeVerification,
} = require("./serial_trace_processor");

const passed = parseVerifierOutput([
  "outcome=complete_pass",
  "total_actions=12",
  "verified=12",
  "unsupported=0",
  "unexpected_diffs=0",
  "seed_start.first_boundary.category=none",
].join("\n"), 0);
assert.strictEqual(passed.ok, true);
assert.strictEqual(passed.totalActions, 12);
assert.strictEqual(passed.boundary.path, null);

const failed = parseVerifierOutput([
  "outcome=failed",
  "total_actions=42",
  "verified=17",
  "unsupported=1",
  "unexpected_diffs=0",
  "seed_start.first_boundary.path=$.actions[step=18].command",
  "seed_start.first_boundary.category=unexpected_sim_real_diff",
  "seed_start.first_boundary.reason=example mismatch",
].join("\n"), 1);
assert.strictEqual(failed.ok, false);
assert.strictEqual(failed.boundary.category, "unexpected_sim_real_diff");
assert.strictEqual(summarizeVerification(failed).boundary.path, "$.actions[step=18].command");

assert.deepStrictEqual(parseArgs(["--max-traces", "2", "--no-repair"]), {
  traceRoot: require("path").resolve(__dirname, "..", "..", "random_traces_loop"),
  verifier: require("path").resolve(__dirname, "..", "..", "simulator", "target", "debug", "sts_verify"),
  maxTraces: 2,
  maxRepairAttempts: Infinity,
  repairAgents: 3,
  repairTimeoutMs: 15 * 60 * 1000,
  noRepair: true,
  retryBlocked: true,
});

assert.deepStrictEqual(parseArgs(["--repair-agents", "3", "--max-repair-attempts", "2", "--skip-blocked"]), {
  traceRoot: require("path").resolve(__dirname, "..", "..", "random_traces_loop"),
  verifier: require("path").resolve(__dirname, "..", "..", "simulator", "target", "debug", "sts_verify"),
  maxTraces: null,
  maxRepairAttempts: 2,
  repairAgents: 3,
  repairTimeoutMs: 15 * 60 * 1000,
  noRepair: false,
  retryBlocked: false,
});

console.log("serial trace processor tests passed");
