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

function controlRequest(port, payload) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: "127.0.0.1", port });
    let buffer = "";
    socket.setEncoding("utf8");
    socket.on("connect", () => {
      socket.write(`${JSON.stringify(payload)}\n`);
    });
    socket.on("data", (chunk) => {
      buffer += chunk;
      const lineEnd = buffer.indexOf("\n");
      if (lineEnd >= 0) {
        const line = buffer.slice(0, lineEnd);
        socket.end();
        try {
          resolve(JSON.parse(line));
        } catch (error) {
          reject(error);
        }
      }
    });
    socket.on("error", reject);
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

    const stale = await controlRequest(port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: "not-current",
    });
    assert.strictEqual(stale.ok, false);
    assert.match(stale.error, /expected_state_id/);

    const accepted = await controlRequest(port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      metadata: { source: "tcp-test" },
    });
    assert.strictEqual(accepted.ok, true);
    assert.strictEqual(accepted.accepted_state_id, liveState.state_id);

    await waitFor(() => stdout.includes("CHOOSE 0\n"));
    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1, stderr);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const action = records.find((record) => record.type === "action");
    assert.ok(action, `missing action record; stderr=${stderr}`);
    assert.strictEqual(action.command, "CHOOSE 0");
    assert.strictEqual(action.command_meta.protocol, "tcp-jsonl");
    assert.strictEqual(action.command_meta.source_state_id, liveState.state_id);
    assert.deepStrictEqual(action.command_meta.metadata, { source: "tcp-test" });
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

Promise.resolve()
  .then(testCommandMetadataIsPreservedInTraceActions)
  .then(testTcpControlRejectsStaleAndAcceptsGuardedCommand)
  .then(() => {
    console.log("trace_client tests passed");
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
