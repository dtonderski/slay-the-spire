#!/usr/bin/env node

const fs = require("fs");
const crypto = require("crypto");
const net = require("net");
const path = require("path");
const readline = require("readline");

const sessionDir = process.env.TRACE_SESSION_DIR
  ? path.resolve(process.env.TRACE_SESSION_DIR)
  : path.join(__dirname, "session");
const outDir = process.env.TRACE_OUT_DIR
  ? path.resolve(process.env.TRACE_OUT_DIR)
  : sessionDir;
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

const tracePath = process.env.TRACE_OUT_DIR
  ? path.join(outDir, `trace-${new Date().toISOString().replace(/[:.]/g, "-")}.jsonl`)
  : path.join(sessionDir, "raw_bridge_current.jsonl");
const clientPid = process.pid;
const logStream = fs.createWriteStream(tracePath, {
  flags: process.env.TRACE_OUT_DIR ? "a" : "w",
});

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
let commandInFlight = null;
const deferredJsonWrites = new Map();

function writeRecord(record) {
  logStream.write(`${JSON.stringify(record)}\n`);
}

function sleepSync(milliseconds) {
  Atomics.wait(
    new Int32Array(new SharedArrayBuffer(4)),
    0,
    0,
    milliseconds,
  );
}

function renameSyncWithRetry(source, destination, attempts = 20) {
  for (let attempt = 1; ; attempt += 1) {
    try {
      fs.renameSync(source, destination);
      return;
    } catch (error) {
      const retryable = new Set(["EPERM", "EACCES", "EBUSY"]).has(error.code);
      if (!retryable || attempt >= attempts) throw error;
      sleepSync(Math.min(100, 5 * (2 ** Math.min(attempt - 1, 4))));
    }
  }
}

function writeJson(filePath, value) {
  const temporaryPath = `${filePath}.${clientPid}.tmp`;
  fs.writeFileSync(temporaryPath, `${JSON.stringify(value, null, 2)}\n`);
  renameSyncWithRetry(temporaryPath, filePath);
}

function writeJsonDeferred(filePath, value) {
  let state = deferredJsonWrites.get(filePath);
  if (!state) {
    state = { writing: false, pending: null };
    deferredJsonWrites.set(filePath, state);
  }
  state.pending = `${JSON.stringify(value, null, 2)}\n`;
  if (!state.writing) drainDeferredJsonWrite(filePath, state);
}

function drainDeferredJsonWrite(filePath, state) {
  if (state.pending === null) {
    state.writing = false;
    return;
  }
  const content = state.pending;
  state.pending = null;
  state.writing = true;
  const temporaryPath = `${filePath}.${clientPid}.tmp`;
  fs.writeFile(temporaryPath, content, (error) => {
    if (error) {
      process.stderr.write(`Failed to publish ${filePath}: ${error.message}\n`);
      drainDeferredJsonWrite(filePath, state);
      return;
    }
    fs.rename(temporaryPath, filePath, (renameError) => {
      if (renameError) {
        process.stderr.write(`Failed to publish ${filePath}: ${renameError.message}\n`);
      }
      drainDeferredJsonWrite(filePath, state);
    });
  });
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
  const controller = controlOwner
    ? {
      owner_id: controlOwner.owner_id,
      acquired_at: controlOwner.acquired_at,
      lease_age_seconds: Math.max(0, (Date.now() - controlOwner.acquired_at_ms) / 1000),
    }
    : null;
  latestStatus = {
    ...value,
    control: controlAddress,
    controller,
  };
  writeJson(statusPath, latestStatus);
}

function writePendingStatus() {
  if (!latestStatus) return;
  const queued = queuedCommands[0] || null;
  writeStatus({
    ...latestStatus,
    pending_command: queuedCommands.length > 0 || Boolean(commandInFlight),
    queued_command: queued?.command ?? null,
    queued_command_meta: queued?.command_meta ?? null,
    command_in_flight: commandInFlight,
  });
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
  try {
    const publishedStatus = JSON.parse(fs.readFileSync(statusPath, "utf8"));
    if (publishedStatus.client_pid !== undefined && publishedStatus.client_pid !== clientPid) {
      return;
    }
  } catch {
    // A missing or interrupted old status still needs an explicit exit record.
  }
  writeStatus({
    step,
    client_pid: clientPid,
    status: "exited",
    reason,
    trace_path: tracePath,
    ended_at: endedAt,
    ...details,
  });
}

function summarize(message) {
  const gs = message.game_state ?? {};
  const screenState = gs.screen_state ?? {};
  const combat = gs.combat_state ?? null;
  const potions = gs.potions ?? [];
  const occupiedPotions = potions
    .map((potion, index) => ({ potion, index }))
    .filter(({ potion }) => {
      const name = String(potion.name ?? "");
      const id = String(potion.id ?? "");
      return name.toLowerCase() !== "potion slot" && id.toLowerCase() !== "potion slot";
    });
  const openPotionSlots = potions.length - occupiedPotions.length;
  const summary = {
    step,
    client_pid: clientPid,
    type: message.type ?? null,
    profile: message.type === "profile" ? message.profile ?? null : null,
    error: message.error ?? null,
    available_commands: message.available_commands ?? [],
    in_game: message.in_game ?? false,
    ready_for_command: message.ready_for_command ?? false,
    boundary_schema: message.boundary_schema ?? null,
    boundary_kind: message.boundary_kind ?? null,
    game_update_seq: message.game_update_seq ?? null,
    dungeon_update_seq: message.dungeon_update_seq ?? null,
    command_execution_seq: message.command_execution_seq ?? null,
    current_action: message.current_action ?? gs.current_action ?? null,
    current_action_instance: message.current_action_instance ?? null,
    current_action_update_count: message.current_action_update_count ?? null,
    actions_queued: message.actions_queued ?? null,
    card_queue_size: message.card_queue_size ?? null,
    end_turn_queued: message.end_turn_queued ?? null,
    pre_turn_actions_size: message.pre_turn_actions_size ?? null,
    effects_size: message.effects_size ?? null,
    top_level_effects_size: message.top_level_effects_size ?? null,
    queued_top_level_effects_size: message.queued_top_level_effects_size ?? null,
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
    potions: occupiedPotions.map(({ potion, index }) => ({
      index,
      name: potion.name,
      id: potion.id,
      can_use: potion.can_use,
      can_discard: potion.can_discard,
      requires_target: potion.requires_target,
    })),
    potion_capacity: potions.length,
    open_potion_slots: openPotionSlots,
    choices: gs.choice_list ?? null,
    shop_potions: (screenState.potions ?? []).map((potion) => ({
      id: potion.id ?? null,
      name: potion.name ?? null,
      price: potion.price ?? null,
    })),
  };

  if (combat) {
    const playerPowers = Array.isArray(combat.player?.powers)
      ? combat.player.powers
      : [];
    const strengthPower = playerPowers.find(
      (power) => String(power?.id ?? power?.name ?? "").toLowerCase() === "strength",
    );
    const playerStrength = Number.isFinite(Number(strengthPower?.amount))
      ? Number(strengthPower.amount)
      : 0;
    summary.combat = {
      turn: combat.turn,
      energy: combat.player?.energy ?? null,
      player_hp: combat.player?.current_hp ?? null,
      player_block: combat.player?.block ?? null,
      player_strength: playerStrength,
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

const GAMEPLAY_BOUNDARY_KINDS = new Set(["interaction_ready", "quiescent", "terminal"]);
const SUPPORTED_BOUNDARY_SCHEMAS = new Set([1, 2, 3, 4, 5, 6]);

function stateCompletesCommand(command, summary, acceptedCommandExecutionSeq = null) {
  if (summary?.error) return true;
  const verb = String(command ?? "").trim().split(/\s+/)[0].toLowerCase();
  if (verb === "profile") return summary?.type === "profile";
  if (!SUPPORTED_BOUNDARY_SCHEMAS.has(summary?.boundary_schema)) return false;
  if (
    summary.boundary_schema >= 2
    && summary.end_turn_queued !== false
    && summary.boundary_kind !== "interaction_ready"
  ) {
    return false;
  }
  if (
    summary.boundary_schema >= 6
    && (summary.effects_size !== 0
      || summary.top_level_effects_size !== 0
      || summary.queued_top_level_effects_size !== 0)
  ) {
    return false;
  }
  if (verb === "state") return summary.boundary_kind === "poll";
  if (
    summary.boundary_schema >= 5
    && (!Number.isInteger(summary.command_execution_seq)
      || !Number.isInteger(acceptedCommandExecutionSeq)
      || summary.command_execution_seq <= acceptedCommandExecutionSeq)
  ) {
    return false;
  }
  return GAMEPLAY_BOUNDARY_KINDS.has(summary?.boundary_kind);
}

function publishState(message) {
  const auxiliaryProfile = message?.type === "profile";
  const previousState = latestState;
  const previousSummary = latestSummary;
  const summary = summarize(message);
  stateSeq += 1;
  if (
    commandInFlight
    && stateSeq > commandInFlight.accepted_state_seq
    && stateCompletesCommand(
      commandInFlight.command,
      summary,
      commandInFlight.accepted_command_execution_seq,
    )
  ) {
    commandInFlight = null;
  }
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
  writeJsonDeferred(statePath, {
    ...latestState,
  });
  writeJsonDeferred(summaryPath, latestSummary);
  notifyStateWaiters();
  const publishedSummary = latestSummary;
  if (auxiliaryProfile) {
    const restoredStateId = previousSummary?.state_id ?? previousState?.state_id ?? null;
    latestState = previousState
      ? { ...previousState, state_seq: stateSeq, state_id: restoredStateId }
      : null;
    latestSummary = previousSummary
      ? { ...previousSummary, state_seq: stateSeq, state_id: restoredStateId }
      : null;
    if (latestState) writeJsonDeferred(statePath, latestState);
    if (latestSummary) writeJsonDeferred(summaryPath, latestSummary);
  }
  return publishedSummary;
}

function notifyStateWaiters() {
  for (let index = stateWaiters.length - 1; index >= 0; index -= 1) {
    const waiter = stateWaiters[index];
    if (stateSeq <= waiter.afterSeq) continue;
    const current = currentProtocolState();
    if (!waiter.accept(current)) continue;
    stateWaiters.splice(index, 1);
    waiter.resolve(current);
  }
}

function waitForStateAfterSeq(afterSeq, timeoutMs, accept = () => true) {
  const current = currentProtocolState();
  if (stateSeq > afterSeq && accept(current)) {
    return Promise.resolve(current);
  }
  return new Promise((resolve) => {
    const waiter = {
      afterSeq,
      accept,
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

function cancelQueuedCommand(commandId, reason = "observed_update_timeout") {
  const index = queuedCommands.findIndex(
    (item) => item.command_meta?.command_id === commandId,
  );
  if (index < 0) return false;
  const [cancelled] = queuedCommands.splice(index, 1);
  writeRecord({
    type: "metadata",
    event: "queued_command_cancelled",
    step,
    cancelled_at: new Date().toISOString(),
    command: cancelled.command,
    command_id: commandId,
    reason,
  });
  return true;
}

function cancelOrphanedQueuedCommands(maxAgeMs, nowMs = Date.now()) {
  if (!Number.isFinite(maxAgeMs) || maxAgeMs < 0 || controlOwner) return [];
  const cancelled = [];
  for (const item of [...queuedCommands]) {
    const submittedAtMs = Number(item.command_meta?.submitted_at) * 1000;
    if (!Number.isFinite(submittedAtMs) || nowMs - submittedAtMs < maxAgeMs) continue;
    const commandId = item.command_meta?.command_id;
    if (commandId && cancelQueuedCommand(commandId, "orphaned_controller_takeover")) {
      cancelled.push(commandId);
    }
  }
  if (cancelled.length > 0) writePendingStatus();
  return cancelled;
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

function writeAction(command, commandMeta) {
  const actionRecord = { type: "action", step, sent_at: new Date().toISOString(), command };
  if (commandMeta) {
    actionRecord.command_meta = commandMeta;
  }
  writeRecord(actionRecord);
  if (commandMeta) {
    writeRecord({
      type: "metadata",
      event: "command_sent",
      step,
      command,
      command_meta: commandMeta,
      sent_at: new Date().toISOString(),
    });
  } else {
    writeRecord({
      type: "metadata",
      event: "legacy_command_sent",
      step,
      command,
      sent_at: new Date().toISOString(),
    });
  }
  process.stderr.write(`[step ${step}] ${command}\n`);
  process.stdout.write(`${command}\n`);
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
    pending_command: queuedCommands.length > 0 || Boolean(commandInFlight),
    command_in_flight: commandInFlight,
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

function controlOwnerLeaseAgeMs(nowMs = Date.now()) {
  if (!controlOwner) return null;
  return Math.max(0, nowMs - controlOwner.acquired_at_ms);
}

function validateProtocolCommand(payload) {
  const command = String(payload.command ?? "").trim();
  if (!command) return "command is required";
  if (command.length > 200) return "command is too long";
  const verb = command.split(/\s+/)[0].toLowerCase();
  const isStartCommand = verb === "start" || verb === "start_verify";
  const startupReady = !latestSummary && latestStatus?.status === "ready";
  const startupStart = isStartCommand && startupReady;
  const startupState = verb === "state" && startupReady;
  const available = new Set((latestSummary?.available_commands ?? []).map((item) => String(item).toLowerCase()));
  const menuStart =
    isStartCommand &&
    latestSummary?.in_game === false &&
    available.has(verb);
  if (!latestSummary && !startupStart && !startupState) return "no observed state is available";
  if (queuedCommands.length > 0) return "a command is already queued";
  if (commandInFlight) return "a command is already in flight";
  if (verb !== "state" && !startupStart && !payload.expected_state_id) {
    return "expected_state_id is required";
  }
  if (verb !== "state" && !startupStart && (payload.expected_state_seq === undefined || payload.expected_state_seq === null)) {
    return "expected_state_seq is required";
  }
  if (payload.expected_state_id && !startupStart && payload.expected_state_id !== latestSummary.state_id) {
    return "expected_state_id does not match current state";
  }
  if (!startupStart && payload.expected_state_seq !== undefined && Number(payload.expected_state_seq) !== stateSeq) {
    return "expected_state_seq does not match current state";
  }
  if (controlOwner && payload.owner_token !== controlOwner.owner_token) {
    return "controller owner_token is required";
  }
  if (verb !== "state" && !startupStart && !menuStart && latestSummary.ready_for_command !== true) {
    return "bridge is not ready for a command";
  }
  if (verb !== "state" && !startupStart && !available.has(verb)) {
    return `command "${verb}" is not available`;
  }
  return null;
}

function validateAbandonRun(payload) {
  if (!latestSummary) return "no observed state is available";
  if (queuedCommands.length > 0) return "a command is already queued";
  if (commandInFlight) return "a command is already in flight";
  if (controlOwner && payload.owner_token !== controlOwner.owner_token) {
    return "controller owner_token is required";
  }
  const available = new Set(
    (latestSummary.available_commands ?? []).map((item) => String(item).toLowerCase()),
  );
  if (latestSummary.ready_for_command !== true && !available.has("abandon")) {
    return "bridge is not ready for a command";
  }
  return null;
}

async function enqueueAbandonRun(payload) {
  const error = validateAbandonRun(payload);
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
    command: "ABANDON",
    source_state_id: latestSummary?.state_id ?? null,
    source_state_seq: stateSeq,
    source_command_execution_seq: Number(latestSummary?.command_execution_seq ?? 0),
    submitted_at: Date.now() / 1000,
    protocol: "tcp-jsonl",
    owner_id: controlOwner?.owner_id ?? null,
    operator_control: "abandon_run",
  };
  if (payload.metadata !== undefined) {
    commandMeta.metadata = payload.metadata;
  }
  const acceptedStateSeq = stateSeq;
  const acceptedStateId = latestSummary?.state_id ?? null;
  const acceptedCommandExecutionSeq = Number(latestSummary?.command_execution_seq ?? 0);
  commandInFlight = {
    command_id: commandId,
    command: commandMeta.command,
    accepted_state_id: acceptedStateId,
    accepted_state_seq: acceptedStateSeq,
    accepted_command_execution_seq: acceptedCommandExecutionSeq,
    accepted_at: new Date().toISOString(),
    operator_control: "abandon_run",
  };
  writeRecord({
    type: "metadata",
    event: "run_abandoned",
    step,
    accepted_at: commandInFlight.accepted_at,
    command: commandMeta.command,
    command_id: commandId,
    command_meta: commandMeta,
  });
  writeRecord({
    type: "command_accept",
    step,
    accepted_at: commandInFlight.accepted_at,
    command: commandMeta.command,
    command_meta: commandMeta,
    accepted_state_id: acceptedStateId,
    accepted_state_seq: acceptedStateSeq,
    accepted_command_execution_seq: acceptedCommandExecutionSeq,
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
    const timeoutMs = Math.max(1, Math.min(30000, Number(payload.update_timeout_ms ?? 10000)));
    const observed = await waitForStateAfterSeq(
      acceptedStateSeq,
      timeoutMs,
      (state) => stateCompletesCommand(
        commandMeta.command,
        state.summary,
        acceptedCommandExecutionSeq,
      ),
    );
    const observedChanged = observed ? observed.state_id !== acceptedStateId : false;
    response.observed_update = observed
      ? {
        ok: true,
        state_id: observed.state_id,
        state_seq: observed.state_seq,
        step: observed.step,
        observed_changed: observedChanged,
        application_status: observedChanged ? "changed" : "unchanged",
        state: observed,
      }
      : {
        ok: false,
        error: "timed out waiting for observed state update",
        accepted_state_id: acceptedStateId,
        accepted_state_seq: acceptedStateSeq,
        observed_changed: false,
        application_status: "timeout",
        step,
      };
    if (!observed) {
      const cancelled = cancelQueuedCommand(commandId);
      writeRecord({
        type: "command_observed_timeout",
        step,
        timed_out_at: new Date().toISOString(),
        command: commandMeta.command,
        command_id: commandId,
        accepted_state_id: acceptedStateId,
        accepted_state_seq: acceptedStateSeq,
        command_cancelled_before_dispatch: cancelled,
      });
      if (cancelled && commandInFlight?.command_id === commandId) {
        commandInFlight = null;
        writePendingStatus();
      }
    }
  }
  return response;
}

async function handleControlMessage(payload) {
  const type = String(payload.type ?? "");
  if (type === "hello") {
    return { ok: true, protocol: "sts-bridge-jsonl-v1", client_pid: clientPid, trace_path: tracePath };
  }
  if (type === "acquire") {
    const ownerId = String(payload.owner_id ?? "").trim();
    if (!ownerId) return { ok: false, error: "owner_id is required" };
    const cancelledOrphanedCommandIds = cancelOrphanedQueuedCommands(
      Number(payload.cancel_orphaned_command_after_ms),
    );
    if (controlOwner && controlOwner.owner_id !== ownerId) {
      const leaseAgeMs = controlOwnerLeaseAgeMs();
      const takeoverAfterMs = Number(payload.takeover_if_stale_after_ms);
      if (
        Number.isFinite(takeoverAfterMs)
        && takeoverAfterMs >= 0
        && leaseAgeMs !== null
        && leaseAgeMs > takeoverAfterMs
      ) {
        const replacedOwnerId = controlOwner.owner_id;
        const replacedLeaseAgeMs = leaseAgeMs;
        controlOwner = {
          owner_id: ownerId,
          owner_token: crypto.randomUUID(),
          acquired_at: new Date().toISOString(),
          acquired_at_ms: Date.now(),
          replaced_owner_id: replacedOwnerId,
        };
        writeRecord({
          type: "metadata",
          event: "controller_takeover",
          replaced_owner_id: replacedOwnerId,
          owner_id: ownerId,
          replaced_lease_age_ms: replacedLeaseAgeMs,
          takeover_if_stale_after_ms: takeoverAfterMs,
          at: controlOwner.acquired_at,
        });
        if (latestStatus) writeStatus(latestStatus);
        return {
          ok: true,
          protocol: "sts-bridge-jsonl-v1",
          owner_id: controlOwner.owner_id,
          owner_token: controlOwner.owner_token,
          replaced_owner_id: replacedOwnerId,
          takeover: true,
          cancelled_orphaned_command_ids: cancelledOrphanedCommandIds,
          state_id: latestSummary?.state_id ?? null,
          state_seq: stateSeq,
        };
      }
      return {
        ok: false,
        error: "bridge is already owned by another controller",
        owner_id: controlOwner.owner_id,
        lease_age_ms: leaseAgeMs,
      };
    }
    if (!controlOwner) {
      controlOwner = {
        owner_id: ownerId,
        owner_token: crypto.randomUUID(),
        acquired_at: new Date().toISOString(),
        acquired_at_ms: Date.now(),
      };
      if (latestStatus) writeStatus(latestStatus);
    }
    return {
      ok: true,
      protocol: "sts-bridge-jsonl-v1",
      owner_id: controlOwner.owner_id,
      owner_token: controlOwner.owner_token,
      cancelled_orphaned_command_ids: cancelledOrphanedCommandIds,
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
  if (type === "abandon_run") {
    return enqueueAbandonRun(payload);
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
    const command = String(payload.command).trim();
    const verb = command.split(/\s+/)[0].toLowerCase();
    const startupStart = (verb === "start" || verb === "start_verify")
      && !latestSummary
      && latestStatus?.status === "ready";
    const startupState = verb === "state" && !latestSummary && latestStatus?.status === "ready";
    const commandMeta = {
      command_id: commandId,
      command,
      source_state_id: startupStart ? null : payload.expected_state_id ?? latestSummary?.state_id ?? null,
      source_state_seq: startupStart ? stateSeq : payload.expected_state_seq ?? stateSeq,
      source_command_execution_seq: Number(latestSummary?.command_execution_seq ?? 0),
      submitted_at: Date.now() / 1000,
      protocol: "tcp-jsonl",
      owner_id: controlOwner?.owner_id ?? null,
    };
    if (payload.metadata !== undefined) {
      commandMeta.metadata = payload.metadata;
    }
    const acceptedStateSeq = stateSeq;
    const acceptedStateId = latestSummary?.state_id ?? null;
    const acceptedCommandExecutionSeq = Number(latestSummary?.command_execution_seq ?? 0);
    commandInFlight = {
      command_id: commandId,
      command: commandMeta.command,
      accepted_state_id: acceptedStateId,
      accepted_state_seq: acceptedStateSeq,
      accepted_command_execution_seq: acceptedCommandExecutionSeq,
      accepted_at: new Date().toISOString(),
    };
    writeRecord({
      type: "command_accept",
      step,
      accepted_at: new Date().toISOString(),
      command: commandMeta.command,
      command_meta: commandMeta,
      accepted_state_id: acceptedStateId,
      accepted_state_seq: acceptedStateSeq,
      accepted_command_execution_seq: acceptedCommandExecutionSeq,
    });
    if (startupStart || startupState) {
      writeAction(commandMeta.command, commandMeta);
    } else {
      enqueueCommand(commandMeta.command, commandMeta);
    }
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
      const observed = await waitForStateAfterSeq(
        acceptedStateSeq,
        timeoutMs,
        (state) => stateCompletesCommand(
          commandMeta.command,
          state.summary,
          acceptedCommandExecutionSeq,
        ),
      );
      const observedChanged = observed ? observed.state_id !== acceptedStateId : false;
      response.observed_update = observed
        ? {
          ok: true,
          state_id: observed.state_id,
          state_seq: observed.state_seq,
          step: observed.step,
          observed_changed: observedChanged,
          application_status: observedChanged ? "changed" : "unchanged",
          state: observed,
        }
        : {
          ok: false,
          error: "timed out waiting for observed state update",
          accepted_state_id: acceptedStateId,
          accepted_state_seq: acceptedStateSeq,
          observed_changed: false,
          application_status: "timeout",
          step,
        };
      if (!observed) {
        const cancelled = cancelQueuedCommand(commandId);
        writeRecord({
          type: "command_observed_timeout",
          step,
          timed_out_at: new Date().toISOString(),
          command: commandMeta.command,
          command_id: commandId,
          accepted_state_id: acceptedStateId,
          accepted_state_seq: acceptedStateSeq,
          command_cancelled_before_dispatch: cancelled,
        });
        if (cancelled && commandInFlight?.command_id === commandId) {
          commandInFlight = null;
          writePendingStatus();
        }
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
    pending_command: Boolean(commandInFlight),
    command_in_flight: commandInFlight,
  });

  // An unsolicited state can overtake the response to the command already in
  // flight. Publish it for diagnostics, but do not let it consume another
  // command or block a later completing boundary in the input queue.
  if (commandInFlight) return null;

  const started = Date.now();
  while (true) {
    if (exiting) return null;
    const elapsedMs = Date.now() - started;
    if (Number.isFinite(autoStateMs) && autoStateMs > 0 && elapsedMs >= autoStateMs) {
      return {
        command: "state",
        command_meta: {
          source: "passive_poll",
          auto_state_ms: autoStateMs,
        },
      };
    }
    const timeoutMs = Math.max(
      1,
      Math.min(100, autoStateMs > 0 ? autoStateMs - elapsedMs : 100),
    );
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

  if (Array.isArray(message.external_rng) && message.external_rng.length > 0) {
    writeRecord({
      type: "external_rng",
      step,
      draws: message.external_rng,
    });
    delete message.external_rng;
  }

  writeRecord({
    type: message.error ? "error" : "state",
    step,
    received_at: new Date().toISOString(),
    message,
  });

  const commandResult = await waitForCommand(message);
  if (commandResult === null) return;
  const command = commandResult.command;
  const commandMeta = commandResult.command_meta;
  step += 1;

  writeStatus({
    step,
    client_pid: clientPid,
    status: "sent",
    trace_path: tracePath,
    command,
    command_meta: commandMeta,
    pending_command: Boolean(commandInFlight),
    command_in_flight: commandInFlight,
    sent_at: new Date().toISOString(),
  });
  writeAction(command, commandMeta);
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
