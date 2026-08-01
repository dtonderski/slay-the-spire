#!/usr/bin/env node

const assert = require("assert");
const {
  promotionCandidate,
} = require("./random_fidelity_corpus_promoter");

const task = {
  fingerprint: "0123456789abcdef",
  boundary: {
    path: "$.actions[step=12].command",
    category: "unexpected_sim_real_diff",
  },
  occurrences: [
    { minimized_trace: "/missing.jsonl" },
    { minimized_trace: "/present.jsonl" },
  ],
};

assert.deepStrictEqual(
  promotionCandidate(task, (candidate) => candidate === "/present.jsonl"),
  {
    minimizedPath: "/present.jsonl",
    fingerprint: "0123456789abcdef",
    boundaryPath: "$.actions[step=12].command",
    boundaryCategory: "unexpected_sim_real_diff",
  },
);
for (const category of [
  "unexpected_seed_start_command",
  "unreconciled_copied_attack_frame",
]) {
  assert.strictEqual(
    promotionCandidate(
      {
        ...task,
        boundary: { path: "$.actions[step=12].command", category },
      },
      (candidate) => candidate === "/present.jsonl",
    ).boundaryCategory,
    category,
  );
}
assert.strictEqual(
  promotionCandidate({
    ...task,
    boundary: { path: "$.verifier", category: "verifier_crash" },
  }),
  null,
);
assert.strictEqual(
  promotionCandidate({
    ...task,
    boundary: { path: "$.trace", category: "trace_integrity_error" },
  }),
  null,
);
assert.strictEqual(promotionCandidate(task, () => false), null);

console.log("random fidelity corpus promoter tests passed");
