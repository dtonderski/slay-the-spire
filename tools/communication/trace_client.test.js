#!/usr/bin/env node

const assert = require("assert");
const { spawn } = require("child_process");
const fs = require("fs");
const net = require("net");
const os = require("os");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..", "..");
const traceClientPath = path.join(__dirname, "trace_client.js");

function waitFor(predicate, timeoutMs = 3000) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    function poll() {
      try {
        const value = predicate();
        if (value) {
          resolve(value);
          return;
        }
      } catch (error) {
        reject(error);
        return;
      }
      if (Date.now() - started > timeoutMs) {
        reject(new Error("timed out waiting for condition"));
        return;
      }
      setTimeout(poll, 25);
    }
    poll();
  });
}

function readJsonLines(filePath) {
  return fs.readFileSync(filePath, "utf8")
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function controlRequest(port, payload, timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: "127.0.0.1", port });
    let buffer = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("timed out waiting for control response"));
    }, timeoutMs);
    socket.setEncoding("utf8");
    socket.on("connect", () => {
      socket.write(`${JSON.stringify(payload)}\n`);
    });
    socket.on("data", (chunk) => {
      buffer += chunk;
      const lineEnd = buffer.indexOf("\n");
      if (lineEnd >= 0) {
        const line = buffer.slice(0, lineEnd);
        clearTimeout(timer);
        socket.end();
        try {
          resolve(JSON.parse(line));
        } catch (error) {
          reject(error);
        }
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

async function testCommandMetadataIsPreservedInTraceActions() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });

  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      TRACE_SESSION_DIR: sessionDir,
      TRACE_OUT_DIR: outDir,
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose"],
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: ["Pray"],
      },
    })}\n`);

    await waitFor(() => fs.existsSync(path.join(sessionDir, "status.json"))
      && JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8")).status === "waiting");

    const metadata = {
      command_id: "cmd-guided-1",
      command: "CHOOSE 0",
      source_state_id: "bridge-state-1",
      metadata: {
        source: "guided_collector",
        collector_id: "collector-1",
      },
    };
    fs.writeFileSync(path.join(sessionDir, "next_command.json"), `${JSON.stringify(metadata)}\n`);
    fs.writeFileSync(path.join(sessionDir, "next_command.txt"), "CHOOSE 0\n");

    await waitFor(() => stdout.includes("CHOOSE 0\n"));
    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const action = records.find((record) => record.type === "action");
    assert.ok(action, `missing action record; stderr=${stderr}`);
    assert.strictEqual(action.command, "CHOOSE 0");
    assert.deepStrictEqual(action.command_meta, metadata);
    assert.strictEqual(fs.existsSync(path.join(sessionDir, "next_command.json")), false);
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testAutoStatePollsAreMarkedAsPassive() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-poll-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });

  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      TRACE_SESSION_DIR: sessionDir,
      TRACE_OUT_DIR: outDir,
      TRACE_AUTO_STATE_MS: "25",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["state"],
      game_state: {
        screen_type: "MAP",
        floor: 1,
      },
    })}\n`);
    await waitFor(() => stdout.includes("ready\nstate\n"), 3000);
    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1, stderr);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const action = records.find((record) => record.type === "action" && record.command === "state");
    assert.ok(action, `missing passive state action; stderr=${stderr}`);
    assert.strictEqual(action.command_meta.source, "passive_poll");
    assert.strictEqual(action.command_meta.auto_state_ms, 25);
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlRejectsStaleAndAcceptsGuardedCommand() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-tcp-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });

  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      TRACE_SESSION_DIR: sessionDir,
      TRACE_OUT_DIR: outDir,
      TRACE_CONTROL_PORT: "0",
      TRACE_AUTO_STATE_MS: "50",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose", "state"],
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: ["Pray"],
      },
    })}\n`);

    const status = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.status === "waiting" && parsed.control?.port ? parsed : null;
    });
    const port = status.control.port;
    const liveState = await controlRequest(port, { type: "state" });
    assert.strictEqual(liveState.ok, true);
    assert.ok(liveState.state_id);
    assert.ok(liveState.state_seq);

    const acquired = await controlRequest(port, {
      type: "acquire",
      owner_id: "test-controller",
    });
    assert.strictEqual(acquired.ok, true);
    assert.strictEqual(acquired.owner_id, "test-controller");
    assert.ok(acquired.owner_token);
    const acquiredStatus = await waitFor(() => {
      const parsed = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
      return parsed.controller?.owner_id === "test-controller" ? parsed : null;
    });
    assert.strictEqual(acquiredStatus.controller.owner_id, "test-controller");
    assert.match(acquiredStatus.controller.acquired_at, /^\d{4}-\d{2}-\d{2}T/);
    assert.strictEqual(typeof acquiredStatus.controller.lease_age_seconds, "number");

    const missingOwner = await controlRequest(port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
    });
    assert.strictEqual(missingOwner.ok, false);
    assert.match(missingOwner.error, /owner_token/);

    const missingStateId = await controlRequest(port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(missingStateId.ok, false);
    assert.match(missingStateId.error, /expected_state_id is required/);

    const missingStateSeq = await controlRequest(port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(missingStateSeq.ok, false);
    assert.match(missingStateSeq.error, /expected_state_seq is required/);

    const stale = await controlRequest(port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: "not-current",
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(stale.ok, false);
    assert.match(stale.error, /expected_state_id/);

    const acceptedPromise = controlRequest(port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
      metadata: { source: "tcp-test" },
      wait_for_state_update: true,
      update_timeout_ms: 3000,
    });
    await waitFor(() => stdout.includes("CHOOSE 0\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["state"],
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: [],
      },
    })}\n`);
    const accepted = await acceptedPromise;
    assert.strictEqual(accepted.ok, true);
    assert.strictEqual(accepted.accepted_state_id, liveState.state_id);
    assert.strictEqual(accepted.accepted_state_seq, liveState.state_seq);
    assert.strictEqual(accepted.observed_update.ok, true);
    assert.notStrictEqual(accepted.observed_update.state_id, liveState.state_id);
    assert.ok(accepted.observed_update.state_seq > liveState.state_seq);
    assert.strictEqual(accepted.observed_update.observed_changed, true);
    assert.strictEqual(accepted.observed_update.application_status, "changed");
    const released = await controlRequest(port, {
      type: "release",
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(released.ok, true);
    assert.strictEqual(released.released, true);
    const releasedStatus = await waitFor(() => {
      const parsed = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
      return parsed.controller === null ? parsed : null;
    });
    assert.strictEqual(releasedStatus.controller, null);

    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1, stderr);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const accept = records.find((record) => record.type === "command_accept");
    assert.ok(accept, `missing command_accept record; stderr=${stderr}`);
    assert.strictEqual(accept.command, "CHOOSE 0");
    assert.strictEqual(accept.command_meta.protocol, "tcp-jsonl");
    assert.strictEqual(accept.accepted_state_id, liveState.state_id);
    assert.strictEqual(accept.accepted_state_seq, liveState.state_seq);
    const action = records.find((record) => record.type === "action");
    assert.ok(action, `missing action record; stderr=${stderr}`);
    assert.strictEqual(action.command, "CHOOSE 0");
    assert.strictEqual(action.command_meta.protocol, "tcp-jsonl");
    assert.strictEqual(action.command_meta.source_state_id, liveState.state_id);
    assert.strictEqual(action.command_meta.source_state_seq, liveState.state_seq);
    assert.strictEqual(action.command_meta.owner_id, "test-controller");
    assert.deepStrictEqual(action.command_meta.metadata, { source: "tcp-test" });
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlDisablesLegacyFileCommandsByDefault() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-tcp-file-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });

  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      TRACE_SESSION_DIR: sessionDir,
      TRACE_OUT_DIR: outDir,
      TRACE_CONTROL_PORT: "0",
      TRACE_AUTO_STATE_MS: "0",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose", "state"],
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: ["Pray"],
      },
    })}\n`);

    const status = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.status === "waiting" && parsed.control?.port ? parsed : null;
    });
    const acquired = await controlRequest(status.control.port, {
      type: "acquire",
      owner_id: "test-controller",
    });
    assert.strictEqual(acquired.ok, true);
    const liveState = await controlRequest(status.control.port, { type: "state" });
    assert.strictEqual(liveState.ok, true);

    fs.writeFileSync(path.join(sessionDir, "next_command.txt"), "CHOOSE 0\n");
    const rejected = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.rejected_command === "CHOOSE 0" ? parsed : null;
    });
    assert.match(rejected.error, /legacy next_command\.txt command rejected/);
    await new Promise((resolve) => setTimeout(resolve, 150));
    assert.strictEqual(stdout.includes("CHOOSE 0\n"), false, stderr);

    const accepted = await controlRequest(status.control.port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(accepted.ok, true);
    await waitFor(() => stdout.includes("CHOOSE 0\n"));

    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlRecordsObservedUpdateTimeout() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-tcp-timeout-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });

  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      TRACE_SESSION_DIR: sessionDir,
      TRACE_OUT_DIR: outDir,
      TRACE_CONTROL_PORT: "0",
      TRACE_AUTO_STATE_MS: "0",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose", "state"],
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: ["Pray"],
      },
    })}\n`);

    const status = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.status === "waiting" && parsed.control?.port ? parsed : null;
    });
    const liveState = await controlRequest(status.control.port, { type: "state" });
    const acquired = await controlRequest(status.control.port, {
      type: "acquire",
      owner_id: "test-controller",
    });

    const accepted = await controlRequest(status.control.port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
      wait_for_state_update: true,
      update_timeout_ms: 50,
    });
    assert.strictEqual(accepted.ok, true);
    assert.strictEqual(accepted.observed_update.ok, false);
    assert.match(accepted.observed_update.error, /timed out/);
    assert.strictEqual(accepted.observed_update.observed_changed, false);
    assert.strictEqual(accepted.observed_update.application_status, "timeout");
    await waitFor(() => stdout.includes("CHOOSE 0\n"));

    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1, stderr);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const accept = records.find((record) => record.type === "command_accept");
    assert.ok(accept, `missing command_accept record; stderr=${stderr}`);
    const timeout = records.find((record) => record.type === "command_observed_timeout");
    assert.ok(timeout, `missing command_observed_timeout record; stderr=${stderr}`);
    assert.strictEqual(timeout.command, "CHOOSE 0");
    assert.strictEqual(timeout.accepted_state_id, liveState.state_id);
    assert.strictEqual(timeout.accepted_state_seq, liveState.state_seq);
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

Promise.resolve()
  .then(testCommandMetadataIsPreservedInTraceActions)
  .then(testAutoStatePollsAreMarkedAsPassive)
  .then(testTcpControlRejectsStaleAndAcceptsGuardedCommand)
  .then(testTcpControlDisablesLegacyFileCommandsByDefault)
  .then(testTcpControlRecordsObservedUpdateTimeout)
  .then(() => {
    console.log("trace_client tests passed");
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
