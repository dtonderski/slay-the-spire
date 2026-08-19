#!/usr/bin/env node

const assert = require("assert");
const { spawn } = require("child_process");
const fs = require("fs");
const net = require("net");
const os = require("os");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..", "..");
const traceClientPath = path.join(__dirname, "trace_client.js");
let currentTest = "startup";

function runTest(test) {
  currentTest = test.name;
  return test();
}

function waitFor(predicate, timeoutMs = 3000, label = "condition") {
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
        reject(new Error(`timed out waiting for ${label}`));
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
    await waitFor(
      () => stdout.includes("ready\n"),
      3000,
      `bridge ready; stdout=${stdout}; stderr=${stderr}`,
    );
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
      && JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8")).status === "waiting",
    3000, "waiting status");

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

    await waitFor(() => stdout.includes("CHOOSE 0\n"), 3000, "CHOOSE output");
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
    await waitFor(() => stdout.includes("ready\n"), 3000, "passive bridge ready");
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["state"],
      game_state: {
        screen_type: "MAP",
        floor: 1,
      },
    })}\n`);
    await waitFor(() => stdout.includes("ready\nstate\n"), 3000, "passive STATE output");
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
      boundary_schema: 2,
      end_turn_queued: false,
      boundary_kind: "interaction_ready",
      game_update_seq: 101,
      dungeon_update_seq: 99,
      current_action: "DiscoveryAction",
      current_action_instance: 4,
      current_action_update_count: 2,
      actions_queued: 1,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: ["Pray"],
        screen_state: {
          potions: [{ id: "GamblersBrew", name: "Gambler's Brew", price: 77 }],
        },
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
    assert.strictEqual(liveState.summary.boundary_schema, 2);
    assert.strictEqual(liveState.summary.end_turn_queued, false);
    assert.strictEqual(liveState.summary.boundary_kind, "interaction_ready");
    assert.strictEqual(liveState.summary.current_action, "DiscoveryAction");
    assert.strictEqual(liveState.summary.current_action_instance, 4);
    assert.strictEqual(liveState.summary.current_action_update_count, 2);
    assert.strictEqual(liveState.summary.actions_queued, 1);
    assert.deepStrictEqual(liveState.summary.shop_potions, [
      { id: "GamblersBrew", name: "Gambler's Brew", price: 77 },
    ]);

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
      boundary_schema: 1,
      boundary_kind: "poll",
      game_update_seq: 102,
      dungeon_update_seq: 100,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: [],
      },
    })}\n`);
    await waitFor(() => {
      const summaryPath = path.join(sessionDir, "summary.json");
      if (!fs.existsSync(summaryPath)) return false;
      return JSON.parse(fs.readFileSync(summaryPath, "utf8")).boundary_kind === "poll";
    });
    const pendingAfterPoll = await controlRequest(port, { type: "state" });
    assert.strictEqual(pendingAfterPoll.pending_command, true);
    assert.strictEqual(pendingAfterPoll.command_in_flight.command, "CHOOSE 0");
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["state"],
      boundary_schema: 2,
      end_turn_queued: false,
      boundary_kind: "quiescent",
      game_update_seq: 103,
      dungeon_update_seq: 101,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
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

async function testTcpControlAllowsExplicitStaleControllerTakeover() {
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
    const first = await controlRequest(port, {
      type: "acquire",
      owner_id: "first-controller",
    });
    assert.strictEqual(first.ok, true);

    const blocked = await controlRequest(port, {
      type: "acquire",
      owner_id: "second-controller",
      takeover_if_stale_after_ms: 60_000,
    });
    assert.strictEqual(blocked.ok, false);
    assert.match(blocked.error, /already owned/);
    assert.strictEqual(blocked.owner_id, "first-controller");
    assert.strictEqual(typeof blocked.lease_age_ms, "number");

    await new Promise((resolve) => setTimeout(resolve, 20));
    const takeover = await controlRequest(port, {
      type: "acquire",
      owner_id: "second-controller",
      takeover_if_stale_after_ms: 0,
    });
    assert.strictEqual(takeover.ok, true);
    assert.strictEqual(takeover.owner_id, "second-controller");
    assert.strictEqual(takeover.replaced_owner_id, "first-controller");
    assert.strictEqual(takeover.takeover, true);
    assert.ok(takeover.owner_token);
    assert.notStrictEqual(takeover.owner_token, first.owner_token);

    const acquiredStatus = await waitFor(() => {
      const parsed = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
      return parsed.controller?.owner_id === "second-controller" ? parsed : null;
    });
    assert.strictEqual(acquiredStatus.controller.owner_id, "second-controller", stderr);
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
    const acceptedStatusPath = path.join(sessionDir, "status.json");
    const sentStatus = await waitFor(() => {
      const parsed = JSON.parse(fs.readFileSync(acceptedStatusPath, "utf8"));
      return parsed.status === "sent" && parsed.command === "CHOOSE 0" ? parsed : null;
    });
    assert.strictEqual(sentStatus.pending_command, true);
    assert.strictEqual(sentStatus.command_in_flight.command, "CHOOSE 0");

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
    const pendingStatus = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
    assert.strictEqual(pendingStatus.pending_command, true);
    assert.strictEqual(pendingStatus.command_in_flight.command, "CHOOSE 0");

    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["state"],
      boundary_schema: 1,
      boundary_kind: "quiescent",
      game_update_seq: 2,
      dungeon_update_seq: 1,
      current_action: null,
      current_action_instance: null,
      current_action_update_count: null,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
      game_state: { screen_type: "EVENT", floor: 2, choice_list: [] },
    })}\n`);
    await waitFor(() => {
      const parsed = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
      return parsed.pending_command === false ? parsed : null;
    });

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

async function testExternalRngIsRecordedAgainstProducingAction() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-rng-"));
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
    await waitFor(() => stdout.includes("ready\n"), 3000, "RNG bridge ready");
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose"],
      game_state: { screen_type: "SHOP", floor: 8 },
    })}\n`);
    await waitFor(() => fs.existsSync(path.join(sessionDir, "status.json"))
      && JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8")).status === "waiting",
    3000, "RNG bridge waiting");
    fs.writeFileSync(path.join(sessionDir, "next_command.txt"), "CHOOSE 0\n");
    await waitFor(() => stdout.includes("CHOOSE 0\n"), 3000, "Courier purchase output");

    const draws = [{
      kind: "card_group_get_random_card_by_type",
      state: {
        state0: "fedcba9876543210",
        state1: "0123456789abcdef",
      },
      range_inclusive: 16,
    }];
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["state"],
      external_rng: draws,
      game_state: { screen_type: "SHOP", floor: 8 },
    })}\n`);

    const tracePath = await waitFor(() => {
      const files = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
      if (files.length !== 1) return null;
      const candidate = path.join(outDir, files[0]);
      return fs.readFileSync(candidate, "utf8").includes("\"type\":\"external_rng\"")
        ? candidate
        : null;
    }, 3000, "external RNG trace record");
    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const records = readJsonLines(tracePath);
    const capture = records.find((record) => record.type === "external_rng");
    assert.ok(capture, `missing external_rng record; stderr=${stderr}`);
    assert.strictEqual(capture.step, 1);
    assert.deepStrictEqual(capture.draws, draws);
    const postState = records.find((record) => record.type === "state" && record.step === 1);
    assert.ok(postState, "missing post-action state");
    assert.strictEqual(postState.message.external_rng, undefined);
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlRejectsAbandonBehindDispatchedCommand() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-tcp-queued-timeout-"));
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
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose", "state"],
      game_state: { screen_type: "COMBAT_REWARD", floor: 13 },
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
    const liveState = await controlRequest(status.control.port, { type: "state" });
    const first = await controlRequest(status.control.port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(first.ok, true);
    await waitFor(() => stdout.includes("CHOOSE 0\n"));

    const abandon = await controlRequest(status.control.port, {
      type: "abandon_run",
      owner_token: acquired.owner_token,
      wait_for_state_update: true,
      update_timeout_ms: 50,
    });
    assert.strictEqual(abandon.ok, false);
    assert.match(abandon.error, /already in flight/);
    const pending = await controlRequest(status.control.port, { type: "state" });
    assert.strictEqual(pending.pending_command, true);

    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose", "state"],
      boundary_schema: 1,
      boundary_kind: "quiescent",
      game_update_seq: 2,
      dungeon_update_seq: 1,
      current_action: null,
      current_action_instance: null,
      current_action_update_count: null,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
      game_state: { screen_type: "COMBAT_REWARD", floor: 13 },
    })}\n`);
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.strictEqual(stdout.includes("ABANDON\n"), false);

    const nextState = await controlRequest(status.control.port, { type: "state" });
    const settle = await controlRequest(status.control.port, {
      type: "command",
      command: "STATE",
      expected_state_id: nextState.state_id,
      expected_state_seq: nextState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(settle.ok, true);
    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlAbandonRunBypassesAvailableCommands() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-tcp-abandon-"));
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
      ready_for_command: false,
      available_commands: ["state", "abandon"],
      game_state: {
        screen_type: "COMBAT",
        floor: 3,
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

    const abandon = controlRequest(status.control.port, {
      type: "abandon_run",
      owner_token: acquired.owner_token,
      metadata: { source: "tcp-test" },
      wait_for_state_update: true,
      update_timeout_ms: 3000,
    });
    await waitFor(() => stdout.includes("ABANDON\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: false,
      ready_for_command: true,
      available_commands: ["start", "state"],
      boundary_schema: 1,
      boundary_kind: "terminal",
      game_update_seq: 2,
      dungeon_update_seq: 0,
      current_action: null,
      current_action_instance: null,
      current_action_update_count: null,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
    })}\n`);
    const result = await abandon;
    assert.strictEqual(result.ok, true);
    assert.strictEqual(result.command, "ABANDON");
    assert.strictEqual(result.accepted_state_id, liveState.state_id);
    assert.strictEqual(result.accepted_state_seq, liveState.state_seq);
    assert.strictEqual(result.observed_update.ok, true);

    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1, stderr);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const abandoned = records.find((record) => record.type === "metadata" && record.event === "run_abandoned");
    assert.ok(abandoned, `missing run_abandoned metadata; stderr=${stderr}`);
    const accept = records.find((record) => record.type === "command_accept" && record.command === "ABANDON");
    assert.ok(accept, `missing ABANDON command_accept; stderr=${stderr}`);
    assert.strictEqual(accept.command_meta.operator_control, "abandon_run");
    const action = records.find((record) => record.type === "action" && record.command === "ABANDON");
    assert.ok(action, `missing ABANDON action; stderr=${stderr}`);
    assert.strictEqual(action.command_meta.operator_control, "abandon_run");
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlProfileRestoresGuardMetadataForNextStart() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-profile-start-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });
  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: { ...process.env, TRACE_SESSION_DIR: sessionDir, TRACE_OUT_DIR: outDir, TRACE_CONTROL_PORT: "0" },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => { stdout += chunk.toString(); });
  child.stderr.on("data", (chunk) => { stderr += chunk.toString(); });
  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: false,
      ready_for_command: true,
      available_commands: ["profile", "start", "state"],
      boundary_schema: 1,
      boundary_kind: "interaction_ready",
      game_update_seq: 1,
      dungeon_update_seq: 0,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
      game_state: { screen_type: "MENU" },
    })}\n`);
    const status = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.status === "waiting" && parsed.control?.port ? parsed : null;
    });
    const beforeProfile = await controlRequest(status.control.port, { type: "state" });
    const acquired = await controlRequest(status.control.port, {
      type: "acquire",
      owner_id: "profile-start-test",
    });
    const profilePromise = controlRequest(status.control.port, {
      type: "command",
      command: "PROFILE",
      expected_state_id: beforeProfile.state_id,
      expected_state_seq: beforeProfile.state_seq,
      owner_token: acquired.owner_token,
      wait_for_state_update: true,
      update_timeout_ms: 3000,
    });
    await waitFor(() => stdout.includes("PROFILE\n"));
    child.stdin.write(`${JSON.stringify({
      type: "profile",
      profile: {
        note_card: "Twin Strike",
        note_upgrades: 1,
        final_act_available: true,
      },
    })}\n`);
    const profile = await profilePromise;
    assert.strictEqual(profile.ok, true, profile.error);
    assert.strictEqual(
      profile.observed_update.state.summary.profile.final_act_available,
      true,
    );

    const restored = await controlRequest(status.control.port, { type: "state" });
    assert.strictEqual(restored.summary.type, null);
    assert.strictEqual(restored.summary.in_game, false);
    assert.strictEqual(restored.state_seq, restored.summary.state_seq);
    assert.strictEqual(restored.state_seq, restored.state.state_seq);
    assert.strictEqual(restored.state_id, restored.summary.state_id);
    assert.strictEqual(restored.state_id, restored.state.state_id);

    const start = await controlRequest(status.control.port, {
      type: "command",
      command: "START IRONCLAD 0 CODEX04",
      expected_state_id: restored.summary.state_id,
      expected_state_seq: restored.summary.state_seq,
      owner_token: acquired.owner_token,
      wait_for_state_update: false,
    });
    assert.strictEqual(start.ok, true, start.error || stderr);
    await waitFor(() => stdout.includes("START IRONCLAD 0 CODEX04\n"));
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlAcceptsAdvertisedStartFromUnreadyMenu() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-tcp-menu-start-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });
  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: { ...process.env, TRACE_SESSION_DIR: sessionDir, TRACE_OUT_DIR: outDir, TRACE_CONTROL_PORT: "0" },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => { stdout += chunk.toString(); });
  child.stderr.on("data", (chunk) => { stderr += chunk.toString(); });
  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: false,
      ready_for_command: false,
      available_commands: ["start", "start_verify", "state"],
    })}\n`);
    const status = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.status === "waiting" && parsed.control?.port ? parsed : null;
    });
    const liveState = await controlRequest(status.control.port, { type: "state" });
    const acquired = await controlRequest(status.control.port, { type: "acquire", owner_id: "test-controller" });
    const accepted = await controlRequest(status.control.port, {
      type: "command",
      command: "START_VERIFY IRONCLAD 0 TESTSEED 10000",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
      wait_for_state_update: false,
    });
    assert.strictEqual(accepted.ok, true, stderr);
    await waitFor(() => stdout.includes("START_VERIFY IRONCLAD 0 TESTSEED 10000\n"));
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlRejectsSecondCommandUntilStateUpdate() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-tcp-in-flight-"));
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
        choice_list: ["Pray", "Leave"],
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

    const firstAccepted = controlRequest(status.control.port, {
      type: "command",
      command: "CHOOSE 0",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
      wait_for_state_update: true,
      update_timeout_ms: 3000,
    });
    await waitFor(() => stdout.includes("CHOOSE 0\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose", "state"],
      boundary_schema: 0,
      boundary_kind: "quiescent",
      game_state: { screen_type: "EVENT", floor: 2, choice_list: [] },
    })}\n`);
    await waitFor(() => {
      const current = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
      return current.pending_command === true && current.summary?.boundary_schema === 0;
    }, 3000, "non-v1 state remains non-completing");

    const overtakenState = await controlRequest(status.control.port, { type: "state" });
    const second = await controlRequest(status.control.port, {
      type: "command",
      command: "CHOOSE 1",
      expected_state_id: overtakenState.state_id,
      expected_state_seq: overtakenState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(second.ok, false);
    assert.match(second.error, /already in flight/);

    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["state"],
      boundary_schema: 1,
      boundary_kind: "quiescent",
      game_update_seq: 2,
      dungeon_update_seq: 1,
      current_action: null,
      current_action_instance: null,
      current_action_update_count: null,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: [],
      },
    })}\n`);
    const first = await firstAccepted;
    assert.strictEqual(first.ok, true);
    assert.strictEqual(first.observed_update.ok, true);

    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1, stderr);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const chooseActions = records.filter((record) => record.type === "action" && /^CHOOSE\b/.test(record.command));
    assert.strictEqual(chooseActions.length, 1);
    assert.strictEqual(chooseActions[0].command, "CHOOSE 0");
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlKeepsDispatchedTimeoutInFlightUntilError() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "corpus");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });

  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      TRACE_SESSION_DIR: sessionDir,
      TRACE_CORPUS_DIR: outDir,
      TRACE_CONTROL_PORT: "0",
      TRACE_ALLOW_FILE_COMMANDS: "0",
      TRACE_AUTO_STATE_MS: "0",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["cancel", "state"],
      game_state: {
        screen_type: "GRID",
        floor: 14,
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

    const cancel = controlRequest(status.control.port, {
      type: "command",
      command: "CANCEL",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
      wait_for_state_update: true,
      update_timeout_ms: 50,
    });
    await waitFor(() => stdout.includes("CANCEL\n"));
    const cancelResult = await cancel;
    assert.strictEqual(cancelResult.ok, true);
    assert.strictEqual(cancelResult.observed_update.ok, false);
    assert.strictEqual(cancelResult.observed_update.application_status, "timeout");

    const blocked = await controlRequest(status.control.port, {
      type: "command",
      command: "STATE",
      expected_state_id: liveState.state_id,
      expected_state_seq: liveState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(blocked.ok, false);
    assert.match(blocked.error, /already in flight/);

    child.stdin.write(`${JSON.stringify({ error: "cancel rejected" })}\n`);
    await waitFor(() => {
      const current = JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"));
      return current.pending_command === false && current.summary?.error ? current : null;
    }, 3000, "matching error releases dispatched timeout");
    const releasedState = await controlRequest(status.control.port, { type: "state" });
    const second = await controlRequest(status.control.port, {
      type: "command",
      command: "STATE",
      expected_state_id: releasedState.state_id,
      expected_state_seq: releasedState.state_seq,
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(second.ok, true, second.error);

    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlAllowsOnlyStateBeforeStartupObservation() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-startup-state-"));
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
  child.stdout.on("data", (chunk) => { stdout += chunk.toString(); });
  try {
    await waitFor(() => stdout.includes("ready\n"));
    const status = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.status === "ready" && parsed.control?.port ? parsed : null;
    });
    const acquired = await controlRequest(status.control.port, {
      type: "acquire",
      owner_id: "startup-state-test",
    });
    const blindGameplay = await controlRequest(status.control.port, {
      type: "command",
      command: "CHOOSE 0",
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(blindGameplay.ok, false);
    assert.match(blindGameplay.error, /no observed state/);

    const statePromise = controlRequest(status.control.port, {
      type: "command",
      command: "STATE",
      owner_token: acquired.owner_token,
      wait_for_state_update: true,
      update_timeout_ms: 3000,
    });
    await waitFor(() => stdout.includes("STATE\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: false,
      ready_for_command: true,
      available_commands: ["profile", "start", "state"],
      boundary_schema: 1,
      boundary_kind: "poll",
      game_update_seq: 1,
      dungeon_update_seq: 0,
      actions_queued: 0,
      card_queue_size: 0,
      pre_turn_actions_size: 0,
      game_state: { screen_type: "MENU" },
    })}\n`);
    const observed = await statePromise;
    assert.strictEqual(observed.ok, true, observed.error);
    assert.strictEqual(observed.observed_update.ok, true);
    assert.strictEqual(observed.observed_update.state.summary.in_game, false);
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTcpControlAllowsStartupStartBeforeObservedState() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-startup-start-"));
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
    const status = await waitFor(() => {
      const statusPath = path.join(sessionDir, "status.json");
      if (!fs.existsSync(statusPath)) return null;
      const parsed = JSON.parse(fs.readFileSync(statusPath, "utf8"));
      return parsed.status === "ready" && parsed.control?.port ? parsed : null;
    });
    const acquired = await controlRequest(status.control.port, {
      type: "acquire",
      owner_id: "test-controller",
    });

    const start = await controlRequest(status.control.port, {
      type: "command",
      command: "START_VERIFY IRONCLAD 0 CODEX04 10000",
      owner_token: acquired.owner_token,
    });
    assert.strictEqual(start.ok, true, start.error);
    assert.strictEqual(start.accepted_state_id, null);
    assert.strictEqual(start.accepted_state_seq, 0);
    await waitFor(() => stdout.includes("START_VERIFY IRONCLAD 0 CODEX04 10000\n"), 3000);

    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1, stderr);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const accept = records.find((record) => record.type === "command_accept" && record.command === "START_VERIFY IRONCLAD 0 CODEX04 10000");
    const action = records.find((record) => record.type === "action" && record.command === "START_VERIFY IRONCLAD 0 CODEX04 10000");
    assert.ok(accept, `missing START_VERIFY command_accept; stderr=${stderr}`);
    assert.ok(action, `missing START_VERIFY action; stderr=${stderr}`);
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

Promise.resolve()
  .then(() => runTest(testCommandMetadataIsPreservedInTraceActions))
  .then(() => runTest(testExternalRngIsRecordedAgainstProducingAction))
  .then(() => runTest(testAutoStatePollsAreMarkedAsPassive))
  .then(() => runTest(testTcpControlRejectsStaleAndAcceptsGuardedCommand))
  .then(() => runTest(testTcpControlAllowsExplicitStaleControllerTakeover))
  .then(() => runTest(testTcpControlDisablesLegacyFileCommandsByDefault))
  .then(() => runTest(testTcpControlRecordsObservedUpdateTimeout))
  .then(() => runTest(testTcpControlRejectsAbandonBehindDispatchedCommand))
  .then(() => runTest(testTcpControlAbandonRunBypassesAvailableCommands))
  .then(() => runTest(testTcpControlProfileRestoresGuardMetadataForNextStart))
  .then(() => runTest(testTcpControlAcceptsAdvertisedStartFromUnreadyMenu))
  .then(() => runTest(testTcpControlRejectsSecondCommandUntilStateUpdate))
  .then(() => runTest(testTcpControlAllowsOnlyStateBeforeStartupObservation))
  .then(() => runTest(testTcpControlAllowsStartupStartBeforeObservedState))
  .then(() => runTest(testTcpControlKeepsDispatchedTimeoutInFlightUntilError))
  .then(() => {
    console.log("trace_client tests passed");
  })
  .catch((error) => {
    console.error(`${currentTest}:`, error);
    process.exitCode = 1;
  });
