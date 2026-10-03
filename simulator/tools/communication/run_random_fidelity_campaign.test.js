#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const crypto = require("crypto");
const { spawnSync } = require("child_process");
const { firstUncollectedPolicySeed } = require("./run_random_fidelity_campaign");

// Synthetic sealing/preflight fixtures, not gameplay or parity evidence.
function fixture(check) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "sts-campaign-resume-"));
  fs.mkdirSync(path.join(dir, "traces"));
  const recordTrace = (prefix, policySeed, metadataPolicySeed = policySeed) => {
    const gameSeed = `${prefix}${String(policySeed).padStart(5, "0")}`;
    const trace = path.join(dir, "traces", `${gameSeed}-p${policySeed}-old.jsonl`);
    fs.writeFileSync(trace, JSON.stringify({ type: "metadata", collection: { policy_seed: metadataPolicySeed, game_seed: gameSeed } }) + "\n");
    fs.appendFileSync(path.join(dir, "ledger.jsonl"), JSON.stringify({
      kind: "collected", terminal_reason: "game_over", game_seed: gameSeed, policy_seed: policySeed, trace,
      trace_sha256: crypto.createHash("sha256").update(fs.readFileSync(trace)).digest("hex"),
    }) + "\n");
    return trace;
  };
  try { check(dir, recordTrace); } finally { fs.rmSync(dir, { recursive: true, force: true }); }
}

fixture((dir, recordTrace) => {
  recordTrace("FIDL", 1); recordTrace("FIDL", 3); recordTrace("OTHER", 1);
  assert.strictEqual(firstUncollectedPolicySeed(dir, "FIDL", 1), 2);
  recordTrace("FIDL", 2);
  assert.strictEqual(firstUncollectedPolicySeed(dir, "FIDL", 1), 4);
  assert.strictEqual(firstUncollectedPolicySeed(dir, "FIDL", 200), 200);
  fs.writeFileSync(path.join(dir, "skipped_policy_seeds.jsonl"), JSON.stringify({ seed_prefix: "FIDL", policy_seed: 4 }) + "\n");
  assert.strictEqual(firstUncollectedPolicySeed(dir, "FIDL", 1), 4);
});
fixture((dir, recordTrace) => {
  recordTrace("FIDL", 1, 999);
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /metadata mismatch/);
});
fixture((dir, recordTrace) => {
  const trace = recordTrace("FIDL", 1);
  fs.appendFileSync(trace, "{}\n");
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /sealed trace hash mismatch/);
});
fixture((dir, recordTrace) => {
  const trace = recordTrace("FIDL", 1);
  const read = fs.readFileSync;
  fs.readFileSync = function (file, ...args) {
    if (file === trace) throw Object.assign(new Error("sealed trace disappeared during hashing"), { code: "ENOENT" });
    return read.call(this, file, ...args);
  };
  try { assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /disappeared during hashing/); }
  finally { fs.readFileSync = read; }
});
fixture((dir, recordTrace) => {
  recordTrace("FIDL", 1);
  const read = fs.readdirSync;
  fs.readdirSync = function (directory, ...args) {
    if (directory === path.join(dir, "traces")) throw Object.assign(new Error("directory disappeared"), { code: "ENOENT" });
    return read.call(this, directory, ...args);
  };
  try { assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /trace directory is missing/); }
  finally { fs.readdirSync = read; }
});
fixture((dir, recordTrace) => {
  fs.unlinkSync(recordTrace("FIDL", 1));
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /sealed trace is missing/);
});
fixture(dir => {
  fs.writeFileSync(path.join(dir, "traces/FIDL00001-p1-malformed.jsonl"), "{malformed\n");
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /unsealed trace/);
});
fixture(dir => {
  fs.writeFileSync(path.join(dir, "ledger.jsonl"), JSON.stringify({ kind: "collected_incomplete", policy_seed: 1, trace: "already-purged.jsonl" }) + "\n");
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /incomplete capture/);
});
fixture(dir => {
  fs.writeFileSync(path.join(dir, "ledger.jsonl"), JSON.stringify({ kind: "collected", terminal_reason: "game_over", trace: "old-unhashed.jsonl" }) + "\n");
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /unsealed collection record/);
});
fixture(dir => {
  fs.writeFileSync(path.join(dir, "campaign_failures.jsonl"), "{}\n");
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /recorded failure/);
});
for (const status of ["failed", "running"]) fixture(dir => {
  fs.writeFileSync(path.join(dir, "campaign_status.json"), JSON.stringify({ status, campaign_pid: -1 }));
  assert.throws(() => firstUncollectedPolicySeed(dir, "FIDL"), /another\/interrupted supervisor/);
});
fixture(dir => {
  const lock = path.join(dir, "campaign.lock"); fs.writeFileSync(lock, "existing-owner\n");
  const run = spawnSync(process.execPath, [path.join(__dirname, "run_random_fidelity_campaign.js")], {
    env: { ...process.env, STS_RANDOM_OUTPUT_DIR: dir, STS_RANDOM_MAX_RUNS: "1", STS_BRIDGE_SESSION_DIR: path.join(dir, "absent") },
    encoding: "utf8", timeout: 3000,
  });
  assert.notEqual(run.status, 0); assert.match(run.stderr, /EEXIST/);
  assert.strictEqual(fs.readFileSync(lock, "utf8"), "existing-owner\n");
  assert.ok(!fs.existsSync(path.join(dir, "campaign_status.json")));
});
console.log("random fidelity campaign tests passed");
