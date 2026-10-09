const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");
const { enumerateGameplayActions, sampleRandomAction } = require("./random_fidelity_collector");

// Shared synthetic fixtures: Java checks actual producer labels/selectability;
// Rust checks the unchanged verifier's binding/projection, not real-game parity.
const fixtures = JSON.parse(fs.readFileSync(path.join(__dirname,
  "../../mods/CommunicationMod/src/test/resources/choice-contract.json"), "utf8"));
const client = fs.readFileSync(path.join(__dirname, "trace_client.js"), "utf8");
const context = vm.createContext({ step: 0, clientPid: 1 });
vm.runInContext(client.slice(client.indexOf("function summarize(message)"),
  client.indexOf("const GAMEPLAY_BOUNDARY_KINDS")), context);

for (const [name, fixture] of Object.entries(fixtures.cases)) {
  test(`producer/client/policy choice contract: ${name}`, () => {
    const summary = JSON.parse(JSON.stringify(context.summarize({
      choice_index_schema: fixtures.choice_index_schema, ...fixture })));
    assert.equal(summary.choice_index_schema, 1);
    assert.deepEqual(summary.choices, fixture.game_state.choice_list);
    assert.deepEqual(summary.selectable_choice_indices, fixture.game_state.selectable_choice_indices);
    assert.deepEqual(enumerateGameplayActions(summary), fixture.commands);
    // Every advertised executable action remains reachable, uniformly indexed.
    for (let i = 0; i < fixture.commands.length; i++) {
      const sample = sampleRandomAction(summary, () => (i + 0.5) / fixture.commands.length);
      assert.equal(sample.command, fixture.commands[i]);
    }
  });
}

test("missing or malformed selectability fails closed, never reindexes offers", () => {
  const summary = { available_commands: ["choose"], screen_type: "COMBAT_REWARD",
    choices: ["potion", "card"] };
  assert.throws(() => enumerateGameplayActions(summary), /selectable_choice_indices required/);
  for (const indices of ["1", [-1], [2], [0, 0], [1, 0], [0.5], [null]]) {
    assert.throws(() => enumerateGameplayActions({ ...summary,
      selectable_choice_indices: indices }), /invalid selectable_choice_indices/);
  }
  assert.throws(() => enumerateGameplayActions({ ...summary,
    selectable_choice_indices: [] }), /choose advertised with no selectable/);
});
