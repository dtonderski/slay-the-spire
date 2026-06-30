#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const readline = require("readline");

const repoRoot = path.resolve(__dirname, "..", "..");
const outDir = path.join(repoRoot, "verification", "corpus", "communication_mod");
const sessionDir = path.join(__dirname, "session");
const commandPath = path.join(sessionDir, "next_command.txt");
const commandMetaPath = path.join(sessionDir, "next_command.json");
const statePath = path.join(sessionDir, "current_state.json");
const summaryPath = path.join(sessionDir, "summary.json");
const statusPath = path.join(sessionDir, "status.json");
const autoStateMs = Number.parseInt(process.env.TRACE_AUTO_STATE_MS ?? "0", 10);
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

function writeRecord(record) {
  logStream.write(`${JSON.stringify(record)}\n`);
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
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
  const endedAt = new Date().toISOString();
  const record = { type: "metadata", event: "exit", reason, ended_at: endedAt, ...details };
  writeRecord(record);
  writeJson(statusPath, {
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
  writeJson(statePath, {
    step,
    client_pid: clientPid,
    received_at: new Date().toISOString(),
    message,
  });
  writeJson(summaryPath, summary);
  return summary;
}

async function waitForCommand(message) {
  const summary = publishState(message);
  writeJson(statusPath, {
    step,
    client_pid: clientPid,
    status: "waiting",
    trace_path: tracePath,
    command_path: commandPath,
    auto_state_ms: autoStateMs,
    summary,
  });

  const started = Date.now();
  while (true) {
    if (fs.existsSync(commandPath)) {
      try {
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
        if (command) {
          return { command, command_meta: commandMeta };
        }
      } catch (error) {
        if (error.code !== "EBUSY" && error.code !== "EPERM") {
          throw error;
        }
      }
    }
    if (Number.isFinite(autoStateMs) && autoStateMs > 0 && Date.now() - started >= autoStateMs) {
      return { command: "state", command_meta: null };
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
  writeJson(statusPath, {
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

writeJson(statusPath, {
  step: 0,
  client_pid: clientPid,
  status: "ready",
  trace_path: tracePath,
});

process.stderr.write(`Bridge ready. Trace: ${tracePath}\n`);
process.stderr.write(`Auto-state polling: ${autoStateMs > 0 ? `${autoStateMs}ms` : "disabled"}\n`);
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
