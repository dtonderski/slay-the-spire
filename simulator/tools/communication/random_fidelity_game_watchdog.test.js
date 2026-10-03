#!/usr/bin/env node
const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const { main } = require("./random_fidelity_game_watchdog");

assert.throws(main, /watchdog is disabled/);
const directory = fs.mkdtempSync(path.join(os.tmpdir(), "sts-disabled-recovery-"));
try {
  // Fake destructive/launch executables make any attempted old recovery visible.
  const called = path.join(directory, "called");
  const poison = path.join(directory, "poison.sh");
  fs.writeFileSync(poison, `#!/bin/sh\nprintf called >> '${called}'\n`, { mode: 0o755 });
  const output = path.join(directory, "must-not-create-output");
  const env = { ...process.env, STS_GAME_JAVA: poison, STS_WINDOWS_TASKLIST: poison,
    STS_WINDOWS_TASKKILL: poison, NODE_BIN: poison, STS_RANDOM_OUTPUT_DIR: output,
    STS_BRIDGE_SESSION_DIR: path.join(directory, "must-not-create-session") };
  for (const [command, args] of [
    [process.execPath, [path.join(__dirname, "random_fidelity_game_watchdog.js")]],
    ["bash", [path.join(__dirname, "collection_overnight_monitor.sh")]],
  ]) {
    const result = spawnSync(command, args, { env, encoding: "utf8", timeout: 3000 });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /disabled/);
    assert.ok(!fs.existsSync(called), "legacy entrypoint attempted recovery");
    assert.ok(!fs.existsSync(output), "legacy entrypoint wrote campaign state");
  }
} finally { fs.rmSync(directory, { recursive: true, force: true }); }
console.log("disabled legacy recovery tests passed");
