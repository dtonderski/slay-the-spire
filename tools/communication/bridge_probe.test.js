#!/usr/bin/env node

const assert = require("assert");
const { bridgeProbeResultFrom } = require("./bridge_probe");

function test(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

test("probe result passes when command is consumed", () => {
  const result = bridgeProbeResultFrom({
    commandExisted: false,
    consumed: true,
    summaryChanged: true,
    statusChanged: true,
    cleanedUp: false,
  });
  assert.strictEqual(result.ok, true);
  assert.deepStrictEqual(result.problems, []);
});

test("probe result fails and records cleanup when command is not consumed", () => {
  const result = bridgeProbeResultFrom({
    commandExisted: false,
    consumed: false,
    summaryChanged: false,
    statusChanged: false,
    cleanedUp: true,
  });
  assert.strictEqual(result.ok, false);
  assert.match(result.problems.join("\n"), /did not consume/);
  assert.strictEqual(result.cleanedUp, true);
});

test("probe result refuses to overwrite an existing command", () => {
  const result = bridgeProbeResultFrom({
    commandExisted: true,
    consumed: false,
    summaryChanged: false,
    statusChanged: false,
    cleanedUp: false,
  });
  assert.strictEqual(result.ok, false);
  assert.match(result.problems.join("\n"), /already exists/);
});
