#!/usr/bin/env node

const assert = require("assert");
const {
  repairWorkerCount,
  restartDelayMs,
  serviceSpecs,
  verifierWorkerCount,
} = require("./run_random_fidelity_pipeline");

assert.strictEqual(verifierWorkerCount("1"), 1);
assert.strictEqual(verifierWorkerCount("8"), 8);
if (process.env.STS_RANDOM_VERIFY_WORKERS === undefined) {
  assert.strictEqual(verifierWorkerCount(), 1);
}
assert.throws(() => verifierWorkerCount("0"), /positive integer/);
assert.throws(() => verifierWorkerCount("nope"), /positive integer/);
assert.strictEqual(repairWorkerCount("1"), 1);
assert.strictEqual(repairWorkerCount("8"), 8);
if (process.env.STS_RANDOM_REPAIR_WORKERS === undefined) {
  assert.strictEqual(repairWorkerCount(), 2);
}
assert.throws(() => repairWorkerCount("0"), /positive integer/);

const specs = serviceSpecs(3, 2);
assert.deepStrictEqual(
  specs.map((spec) => spec.name),
  [
    "campaign",
    "corpus-promoter",
    "verifier-0",
    "verifier-1",
    "verifier-2",
    "repair-integrator",
    "repair-0",
    "repair-1",
  ],
);
assert.strictEqual(specs[0].env.STS_RANDOM_DEFER_VERIFICATION, "1");
assert.strictEqual(specs[4].env.STS_RANDOM_VERIFY_WORKER_INDEX, "2");
assert.strictEqual(specs[6].env.STS_RANDOM_REPAIR_WORKER_INDEX, "0");
assert.strictEqual(specs[7].env.STS_RANDOM_REPAIR_WORKERS, "2");
assert.strictEqual(restartDelayMs(0), 1000);
assert.strictEqual(restartDelayMs(3), 8000);
assert.strictEqual(restartDelayMs(99), 30000);

console.log("random fidelity pipeline supervisor tests passed");
