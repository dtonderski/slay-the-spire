#!/usr/bin/env node

const assert = require("assert");
const {
  parseFailureTraceNames,
} = require("./random_fidelity_batch_gate");

assert.deepStrictEqual(
  parseFailureTraceNames(`
3 permanent traces failed:
random-fidelity-one.jsonl: unexpected diff
trace-two.jsonl: unsupported boundary
random-fidelity-one.jsonl: repeated output
not-a-trace.txt: ignored
path/to/trace-three.jsonl: ignored nested path
`),
  ["random-fidelity-one.jsonl", "trace-two.jsonl"],
);

console.log("random fidelity batch gate tests passed");
