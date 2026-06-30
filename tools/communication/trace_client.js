#!/usr/bin/env node

const fs = require("fs");
const crypto = require("crypto");
const net = require("net");
const path = require("path");
const readline = require("readline");

const repoRoot = path.resolve(__dirname, "..", "..");
const outDir = process.env.TRACE_OUT_DIR
  ? path.resolve(process.env.TRACE_OUT_DIR)
  : path.join(repoRoot, "verification", "corpus", "communication_mod");
const sessionDir = process.env.TRACE_SESSION_DIR
  ? path.resolve(process.env.TRACE_SESSION_DIR)
  : path.join(__dirname, "session");
const commandPath = path.join(sessionDir, "next_command.txt");
const commandMetaPath = path.join(sessionDir, "next_command.json");
const statePath = path.join(sessionDir, "current_state.json");
const summaryPath = path.join(sessionDir, "summary.json");
const statusPath = path.join(sessionDir, "status.json");
const autoStateMs = Number.parseInt(process.env.TRACE_AUTO_STATE_MS ?? "0", 10);
const controlPort = process.env.TRACE_CONTROL_PORT === undefined
  ? null
  : Number.parseInt(process.env.TRACE_CONTROL_PORT, 10);
const allowFileCommands = controlPort === null || process.env.TRACE_ALLOW_FILE_COMMANDS === "1";
let exiting = false;

fs.mkdirSync(outDir, { recursive: true });
fs.mkdirSync(sessionDir, { recursive: true });
if (fs.existsSync(commandPath)) {
  fs.unlinkSync(commandPath);
}
if (fs.existsSync(commandMetaPath)) {
  fs.unlinkSync(commandMetaPath);
}

const tracePath = path.join(
  outDir,
  `trace-${new Date().toISOString().replace(/[:.]/g, "-")}.jsonl`,
);
const clientPid = process.pid;
const logStream = fs.createWriteStream(tracePath, { flags: "a" });

let step = 0;
let processing = false;
const pendingLines = [];
const queuedCommands = [];
const commandWaiters = [];
const stateWaiters = [];
let latestState = null;
let latestSummary = null;
let latestStatus = null;
let controlServer = null;
let controlAddress = null;
let stateSeq = 0;
let controlOwner = null;

function writeRecord(record) {
  logStream.write(`${JSON.stringify(record)}\n`);
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function stateIdFor(message, summary) {
  const encoded = JSON.stringify({
    step,
    message,
    summary,
  });
  return crypto.createHash("sha256").update(encoded).digest("hex").slice(0, 32);
}

function writeStatus(value) {
  latestStatus = {
    ...value,
    control: controlAddress,
    controller: controlOwner
      ? {
        owner_id: controlOwner.owner_id,
        acquired_at: controlOwner.acquired_at,
      }
      : null,
  };
  writeJson(statusPath, latestStatus);
}

function readCommandMeta() {
  if (!fs.existsSync(commandMetaPath)) return null;
  try {
    return JSON.parse(fs.readFileSync(commandMetaPath, "utf8"));
  } catch (error) {
    return { error: error.message };
  }
}

function markExit(reason, details = {}) {
  if (exiting) return;
  exiting = true;
  if (controlServer) {
    controlServer.close();
    controlServer = null;
  }
  const endedAt = new Date().toISOString();
  const record = { type: "metadata", event: "exit", reason, ended_at: endedAt, ...details };
  writeRecord(record);
  writeStatus({
    step,
    status: "exited",
    reason,
    trace_path: tracePath,
    ended_at: endedAt,
    ...details,
  });
}

function summarize(message) {
  const gs = message.game_state ?? {};
  const combat = gs.combat_state ?? null;
  const summary = {
    step,
    client_pid: clientPid,
    error: message.error ?? null,
    available_commands: message.available_commands ?? [],
    in_game: message.in_game ?? false,
    ready_for_command: message.ready_for_command ?? false,
    screen_type: gs.screen_type ?? null,
    screen_name: gs.screen_name ?? null,
    room_phase: gs.room_phase ?? null,
    room_type: gs.room_type ?? null,
    floor: gs.floor ?? null,
    seed: gs.seed ?? null,
    ascension_level: gs.ascension_level ?? null,
    class: gs.class ?? null,
    current_hp: gs.current_hp ?? null,
    max_hp: gs.max_hp ?? null,
    gold: gs.gold ?? null,
    potions: (gs.potions ?? []).map((potion, index) => ({
      index,
      name: potion.name,
      id: potion.id,
      can_use: potion.can_use,
      can_discard: potion.can_discard,
    })),
    choices: gs.choice_list ?? null,
  };

  if (combat) {
    summary.combat = {
      turn: combat.turn,
      energy: combat.player?.energy ?? null,
      player_hp: combat.player?.current_hp ?? null,
      player_block: combat.player?.block ?? null,
      hand: (combat.hand ?? []).map((card, index) => ({
        index: index + 1,
        id: card.id,
        name: card.name,
        cost: card.cost,
        playable: card.is_playable,
        type: card.type,
        has_target: card.has_target,
      })),
      monsters: (combat.monsters ?? []).map((monster, index) => ({
        index,
        id: monster.id,
        name: monster.name,
        hp: monster.current_hp,
        max_hp: monster.max_hp,
        block: monster.block,
        intent: monster.intent,
        gone: monster.is_gone,
        half_dead: monster.half_dead,
      })),
      draw_pile_count: combat.draw_pile?.length ?? 0,
      discard_pile_count: combat.discard_pile?.length ?? 0,
      exhaust_pile_count: combat.exhaust_pile?.length ?? 0,
    };
  }

  return summary;
}

function publishState(message) {
  const summary = summarize(message);
  stateSeq += 1;
  const stateId = stateIdFor(message, summary);
  latestState = {
    step,
    state_seq: stateSeq,
    state_id: stateId,
    client_pid: clientPid,
    trace_path: tracePath,
    received_at: new Date().toISOString(),
    message,
  };
  latestSummary = {
    ...summary,
    state_seq: stateSeq,
    state_id: stateId,
  };
  writeJson(statePath, {
    ...latestState,
  });
  writeJson(summaryPath, latestSummary);
  notifyStateWaiters();
  return latestSummary;
}

function notifyStateWaiters() {
  for (let index = stateWaiters.length - 1; index >= 0; index -= 1) {
    const waiter = stateWaiters[index];
    if (stateSeq <= waiter.afterSeq) continue;
    stateWaiters.splice(index, 1);
    waiter.resolve(currentProtocolState());
  }
}

function waitForStateAfterSeq(afterSeq, timeoutMs) {
  if (stateSeq > afterSeq) {
    return Promise.resolve(currentProtocolState());
  }
  return new Promise((resolve) => {
    const waiter = {
      afterSeq,
      resolve(value) {
        if (timer) clearTimeout(timer);
        resolve(value);
      },
    };
    const timer = timeoutMs > 0
      ? setTimeout(() => {
        const index = stateWaiters.indexOf(waiter);
        if (index >= 0) stateWaiters.splice(index, 1);
        resolve(null);
      }, timeoutMs)
      : null;
    stateWaiters.push(waiter);
  });
}

function enqueueCommand(command, commandMeta) {
  const item = { command, command_meta: commandMeta ?? null };
  if (latestStatus) {
    writeStatus({
      ...latestStatus,
      pending_command: true,
      queued_command: command,
      queued_command_meta: commandMeta ?? null,
    });
  }
  const waiter = commandWaiters.shift();
  if (waiter) {
    waiter(item);
  } else {
    queuedCommands.push(item);
  }
}

function waitForQueuedCommand(timeoutMs) {
  if (queuedCommands.length > 0) {
    return Promise.resolve(queuedCommands.shift());
  }
  return new Promise((resolve) => {
    const timer = timeoutMs > 0
      ? setTimeout(() => {
        const index = commandWaiters.indexOf(waiter);
        if (index >= 0) commandWaiters.splice(index, 1);
        resolve(null);
      }, timeoutMs)
      : null;
    function waiter(value) {
      if (timer) clearTimeout(timer);
      resolve(value);
    }
    commandWaiters.push(waiter);
  });
}

function readAndClearFileCommand() {
  const command = fs.readFileSync(commandPath, "utf8").trim();
  const commandMeta = readCommandMeta();
  try {
    fs.unlinkSync(commandPath);
  } catch (error) {
    if (error.code !== "ENOENT") {
      throw error;
    }
  }
  try {
    if (fs.existsSync(commandMetaPath)) fs.unlinkSync(commandMetaPath);
  } catch (error) {
    if (error.code !== "ENOENT") {
      throw error;
    }
  }
  return { command, command_meta: commandMeta };
}

function rejectLegacyFileCommand(commandResult) {
  const detail = "legacy next_command.txt command rejected because TCP control is enabled";
  writeStatus({
    ...latestStatus,
    status: "waiting",
    error: detail,
    rejected_command: commandResult.command,
    rejected_command_meta: commandResult.command_meta,
  });
  process.stderr.write(`${detail}: ${commandResult.command || "<empty>"}\n`);
}

function currentProtocolState() {
  return {
    ok: true,
    protocol: "sts-bridge-jsonl-v1",
    client_pid: clientPid,
    trace_path: tracePath,
    step,
    state_seq: stateSeq,
    state_id: latestSummary?.state_id ?? null,
    ready_for_command: latestSummary?.ready_for_command ?? false,
    available_commands: latestSummary?.available_commands ?? [],
    pending_command: queuedCommands.length > 0,
    summary: latestSummary,
    state: latestState,
    status: latestStatus,
    controller: controlOwner
      ? {
        owner_id: controlOwner.owner_id,
        acquired_at: controlOwner.acquired_at,
      }
      : null,
  };
}

function validateProtocolCommand(payload) {
  const command = String(payload.command ?? "").trim();
  if (!command) return "command is required";
  if (command.length > 200) return "command is too long";
  if (!latestSummary) return "no observed state is available";
  if (queuedCommands.length > 0) return "a command is already queued";
  const verb = command.split(/\s+/)[0].toLowerCase();
  if (verb !== "state" && !payload.expected_state_id) {
    return "expected_state_id is required";
  }
  if (verb !== "state" && (payload.expected_state_seq === undefined || payload.expected_state_seq === null)) {
    return "expected_state_seq is required";
  }
  if (payload.expected_state_id && payload.expected_state_id !== latestSummary.state_id) {
    return "expected_state_id does not match current state";
  }
  if (payload.expected_state_seq !== undefined && Number(payload.expected_state_seq) !== stateSeq) {
    return "expected_state_seq does not match current state";
  }
  if (controlOwner && payload.owner_token !== controlOwner.owner_token) {
    return "controller owner_token is required";
  }
  const available = new Set((latestSummary.available_commands ?? []).map((item) => String(item).toLowerCase()));
  if (verb !== "state" && latestSummary.ready_for_command !== true) {
    return "bridge is not ready for a command";
  }
  if (verb !== "state" && !available.has(verb)) {
    return `command "${verb}" is not available`;
  }
  return null;
}

async function handleControlMessage(payload) {
  const type = String(payload.type ?? "");
  if (type === "hello") {
    return { ok: true, protocol: "sts-bridge-jsonl-v1", client_pid: clientPid, trace_path: tracePath };
  }
  if (type === "acquire") {
    const ownerId = String(payload.owner_id ?? "").trim();
    if (!ownerId) return { ok: false, error: "owner_id is required" };
    if (controlOwner && controlOwner.owner_id !== ownerId) {
      return {
        ok: false,
        error: "bridge is already owned by another controller",
        owner_id: controlOwner.owner_id,
      };
    }
    if (!controlOwner) {
      controlOwner = {
        owner_id: ownerId,
        owner_token: crypto.randomUUID(),
        acquired_at: new Date().toISOString(),
      };
      if (latestStatus) writeStatus(latestStatus);
    }
    return {
      ok: true,
      protocol: "sts-bridge-jsonl-v1",
      owner_id: controlOwner.owner_id,
      owner_token: controlOwner.owner_token,
      state_id: latestSummary?.state_id ?? null,
      state_seq: stateSeq,
    };
  }
  if (type === "release") {
    if (!controlOwner) return { ok: true, released: false };
    if (payload.owner_token !== controlOwner.owner_token) {
      return { ok: false, error: "owner_token does not match active controller" };
    }
    const ownerId = controlOwner.owner_id;
    controlOwner = null;
    if (latestStatus) writeStatus(latestStatus);
    return { ok: true, released: true, owner_id: ownerId };
  }
  if (type === "state") {
    return currentProtocolState();
  }
  if (type === "command") {
    const error = validateProtocolCommand(payload);
    if (error) {
      return {
        ok: false,
      error,
      state_id: latestSummary?.state_id ?? null,
      state_seq: stateSeq,
      step,
    };
    }
    const commandId = payload.command_id || crypto.randomUUID();
    const commandMeta = {
      command_id: commandId,
      command: String(payload.command).trim(),
      source_state_id: payload.expected_state_id ?? latestSummary?.state_id ?? null,
      source_state_seq: payload.expected_state_seq ?? stateSeq,
      submitted_at: Date.now() / 1000,
      protocol: "tcp-jsonl",
      owner_id: controlOwner?.owner_id ?? null,
    };
    if (payload.metadata !== undefined) {
      commandMeta.metadata = payload.metadata;
    }
    const acceptedStateSeq = stateSeq;
    const acceptedStateId = latestSummary?.state_id ?? null;
    writeRecord({
      type: "command_accept",
      step,
      accepted_at: new Date().toISOString(),
      command: commandMeta.command,
      command_meta: commandMeta,
      accepted_state_id: acceptedStateId,
      accepted_state_seq: acceptedStateSeq,
    });
    enqueueCommand(commandMeta.command, commandMeta);
    const response = {
      ok: true,
      command_id: commandId,
      command: commandMeta.command,
      accepted_state_id: acceptedStateId,
      accepted_state_seq: acceptedStateSeq,
      step,
      state: currentProtocolState(),
    };
    if (payload.wait_for_state_update) {
      const timeoutMs = Math.max(1, Math.min(30000, Number(payload.update_timeout_ms ?? 5000)));
      const observed = await waitForStateAfterSeq(acceptedStateSeq, timeoutMs);
      response.observed_update = observed
        ? {
          ok: true,
          state_id: observed.state_id,
          state_seq: observed.state_seq,
          step: observed.step,
          state: observed,
        }
        : {
          ok: false,
          error: "timed out waiting for observed state update",
          accepted_state_id: acceptedStateId,
          accepted_state_seq: acceptedStateSeq,
          step,
        };
      if (!observed) {
        writeRecord({
          type: "command_observed_timeout",
          step,
          timed_out_at: new Date().toISOString(),
          command: commandMeta.command,
          command_id: commandId,
          accepted_state_id: acceptedStateId,
          accepted_state_seq: acceptedStateSeq,
        });
      }
    }
    return response;
  }
  return { ok: false, error: `unknown control message type "${type}"` };
}

function startControlServer() {
  if (controlPort === null) return;
  if (!Number.isInteger(controlPort) || controlPort < 0 || controlPort > 65535) {
    throw new Error("TRACE_CONTROL_PORT must be an integer TCP port");
  }
  controlServer = net.createServer((socket) => {
    socket.setEncoding("utf8");
    let buffer = "";
    function send(value) {
      socket.write(`${JSON.stringify(value)}\n`);
    }
    socket.on("data", (chunk) => {
      buffer += chunk;
      const lines = buffer.split(/\r?\n/);
      buffer = lines.pop() ?? "";
      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          Promise.resolve(handleControlMessage(JSON.parse(line))).then(send, (error) => {
            send({ ok: false, error: error.message });
          });
        } catch (error) {
          send({ ok: false, error: error.message });
        }
      }
    });
  });
  controlServer.listen(controlPort, "127.0.0.1", () => {
    const address = controlServer.address();
    controlAddress = {
      host: address.address,
      port: address.port,
      protocol: "tcp-jsonl",
    };
    if (latestStatus) writeStatus(latestStatus);
    process.stderr.write(`Control socket: ${controlAddress.host}:${controlAddress.port}\n`);
  });
}

async function waitForCommand(message) {
  const summary = publishState(message);
  writeStatus({
    step,
    client_pid: clientPid,
    status: "waiting",
    trace_path: tracePath,
    command_path: commandPath,
    auto_state_ms: autoStateMs,
    allow_file_commands: allowFileCommands,
    summary,
  });

  const started = Date.now();
  while (true) {
    const timeoutMs = Math.max(0, Math.min(100, autoStateMs > 0 ? autoStateMs - (Date.now() - started) : 100));
    const queued = await waitForQueuedCommand(timeoutMs);
    if (queued) return queued;
    if (fs.existsSync(commandPath)) {
      try {
        const commandResult = readAndClearFileCommand();
        if (commandResult.command) {
          if (!allowFileCommands) {
            rejectLegacyFileCommand(commandResult);
          } else {
            return commandResult;
          }
        }
      } catch (error) {
        if (error.code !== "EBUSY" && error.code !== "EPERM") {
          throw error;
        }
      }
    }
    if (Number.isFinite(autoStateMs) && autoStateMs > 0 && Date.now() - started >= autoStateMs) {
      return {
        command: "state",
        command_meta: {
          source: "passive_poll",
          auto_state_ms: autoStateMs,
        },
      };
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

async function handleLine(line) {
  const raw = line.trim();
  if (!raw) return;

  let message;
  try {
    message = JSON.parse(raw);
  } catch (error) {
    writeRecord({ type: "parse_error", step, raw, error: error.message });
    process.stdout.write("state\n");
    return;
  }

  writeRecord({
    type: message.error ? "error" : "state",
    step,
    received_at: new Date().toISOString(),
    message,
  });

  const commandResult = await waitForCommand(message);
  const command = commandResult.command;
  const commandMeta = commandResult.command_meta;
  step += 1;

  const actionRecord = { type: "action", step, sent_at: new Date().toISOString(), command };
  if (commandMeta) {
    actionRecord.command_meta = commandMeta;
  }
  writeRecord(actionRecord);
  writeStatus({
    step,
    client_pid: clientPid,
    status: "sent",
    trace_path: tracePath,
    command,
    command_meta: commandMeta,
    sent_at: new Date().toISOString(),
  });
  process.stderr.write(`[step ${step}] ${command}\n`);
  process.stdout.write(`${command}\n`);
}

async function drainQueue() {
  if (processing) return;
  processing = true;
  while (pendingLines.length > 0) {
    await handleLine(pendingLines.shift());
  }
  processing = false;
}

writeRecord({
  type: "metadata",
  schema: 1,
  source: "communication_mod",
  client: "tools/communication/trace_client.js",
  client_pid: clientPid,
  started_at: new Date().toISOString(),
});

writeStatus({
  step: 0,
  client_pid: clientPid,
  status: "ready",
  trace_path: tracePath,
});

process.stderr.write(`Bridge ready. Trace: ${tracePath}\n`);
process.stderr.write(`Auto-state polling: ${autoStateMs > 0 ? `${autoStateMs}ms` : "disabled"}\n`);
startControlServer();
process.stdout.write("ready\n");

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on("line", (line) => {
  pendingLines.push(line);
  void drainQueue();
});
rl.on("close", () => {
  markExit("stdin_closed");
});

process.on("exit", () => {
  markExit("process_exit");
  if (controlServer) controlServer.close();
  logStream.end();
});
process.on("uncaughtException", (error) => {
  markExit("uncaught_exception", { error: error.stack ?? error.message });
  process.exitCode = 1;
});
process.on("unhandledRejection", (error) => {
  markExit("unhandled_rejection", { error: String(error?.stack ?? error) });
  process.exitCode = 1;
});
