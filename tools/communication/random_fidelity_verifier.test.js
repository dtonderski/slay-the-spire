#!/usr/bin/env node

const assert = require("assert");
const path = require("path");
const {
  assignedToWorker,
  isVerifierInfrastructureError,
  verificationDisposition,
  verifierPath,
} = require("./random_fidelity_verifier");
const { isPromotableFailure } = require("./random_fidelity_collector");

assert.strictEqual(
  verifierPath,
  path.resolve(__dirname, "..", "..", "simulator", "target", "release", "sts_verify"),
);

for (let queueIndex = 0; queueIndex < 100; queueIndex += 1) {
  const owners = [0, 1, 2].filter((workerIndex) =>
    assignedToWorker(queueIndex, 3, workerIndex),
  );
  assert.deepStrictEqual(owners, [queueIndex % 3]);
}

assert.strictEqual(
  isVerifierInfrastructureError(new Error("spawnSync /tmp/sts_verify EPERM")),
  true,
);
assert.strictEqual(
  isVerifierInfrastructureError(new Error("verifier invocation failed with exit 101: panic")),
  false,
);
assert.strictEqual(
  isPromotableFailure({
    unexpectedDiffs: 1,
    duplicateDispositions: 0,
    boundaryCategory: "none",
    boundaryPath: null,
    firstDiff: { step: 12, command: "PLAY 0", label: "player.energy" },
    diffLines: ["player.energy simulated=1 observed=0"],
  }),
  true,
);
assert.deepStrictEqual(
  verificationDisposition({
    exitCode: 2,
    unexpectedDiffs: 0,
    boundaryCategory: "none",
    duplicateDispositions: 0,
    unresolvedTransientAssertions: 0,
    terminalStateObserved: false,
  }, null, true),
  { failed: false, status: "verified_prefix" },
);
assert.deepStrictEqual(
  verificationDisposition({
    exitCode: 2,
    unexpectedDiffs: 0,
    boundaryCategory: "none",
    duplicateDispositions: 1,
    unresolvedTransientAssertions: 0,
    terminalStateObserved: false,
  }, null, true),
  { failed: false, status: "trace_integrity_error" },
);
assert.deepStrictEqual(
  verificationDisposition({
    exitCode: 2,
    unexpectedDiffs: 1,
    boundaryCategory: "unexpected_sim_real_diff",
    duplicateDispositions: 0,
    unresolvedTransientAssertions: 0,
  }, null, true),
  { failed: true, status: "divergence" },
);

console.log("random_fidelity_verifier tests passed");
