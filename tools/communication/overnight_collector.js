#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..", "..");
const sessionDir = path.join(__dirname, "session");
const commandPath = path.join(sessionDir, "next_command.txt");
const statusPath = path.join(sessionDir, "status.json");
const summaryPath = path.join(sessionDir, "summary.json");
const logPath = path.join(sessionDir, "overnight_collector.log");
const collectorStatePath = path.join(sessionDir, "overnight_collector_state.json");

const seedPrefix = process.env.STS_AUTO_SEED_PREFIX || "M29";
const maxRuns = Number.parseInt(process.env.STS_AUTO_MAX_RUNS || "200", 10);
const tickMs = Number.parseInt(process.env.STS_AUTO_TICK_MS || "500", 10);
const maxStatePolls = Number.parseInt(process.env.STS_AUTO_MAX_STATE_POLLS || "5", 10);
const maxSameCommand = Number.parseInt(process.env.STS_AUTO_MAX_SAME_COMMAND || "2", 10);
const maxIdleMs = Number.parseInt(process.env.STS_AUTO_MAX_IDLE_MS || "120000", 10);

let runIndex = loadRunIndex();
let lastStep = -1;
let lastSignature = "";
let repeatedStatePolls = 0;
let lastCommandSignature = "";
let repeatedSameCommand = 0;

fs.mkdirSync(sessionDir, { recursive: true });

function log(line) {
  const message = `[${new Date().toISOString()}] ${line}`;
  console.log(message);
  fs.appendFileSync(logPath, `${message}\n`);
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function fileAgeMs(filePath) {
  try {
    return Date.now() - fs.statSync(filePath).mtimeMs;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}

function staleSessionReasonFrom({ summary, summaryAgeMs, status, statusAgeMs, maxIdleThresholdMs }) {
  if (!Number.isFinite(maxIdleThresholdMs) || maxIdleThresholdMs <= 0) return null;
  if (!summary && summaryAgeMs > maxIdleThresholdMs) {
    return `missing/stale summary: ageMs=${Math.round(summaryAgeMs)}`;
  }
  if (status?.status === "exited") {
    return `bridge exited: ${status.reason || "unknown"}`;
  }
  if (summaryAgeMs > maxIdleThresholdMs && statusAgeMs > maxIdleThresholdMs) {
    return `session idle: summaryAgeMs=${Math.round(summaryAgeMs)} statusAgeMs=${Math.round(statusAgeMs)}`;
  }
  return null;
}

function staleSessionReason() {
  return staleSessionReasonFrom({
    summary: readJson(summaryPath),
    summaryAgeMs: fileAgeMs(summaryPath),
    status: readJson(statusPath),
    statusAgeMs: fileAgeMs(statusPath),
    maxIdleThresholdMs: maxIdleMs,
  });
}

function loadRunIndex() {
  const explicitStart = process.env.STS_AUTO_START_INDEX;
  if (explicitStart) return Number.parseInt(explicitStart, 10);
  const state = readJson(collectorStatePath);
  return Number.isInteger(state?.next_run_index) ? state.next_run_index : 1;
}

function saveRunIndex() {
  fs.writeFileSync(
    collectorStatePath,
    `${JSON.stringify({ next_run_index: runIndex, updated_at: new Date().toISOString() }, null, 2)}\n`,
  );
}

function commandVerb(command) {
  return String(command || "").trim().split(/\s+/)[0]?.toLowerCase() || "";
}

function commandIsAvailable(summary, command) {
  const verb = commandVerb(command);
  if (verb === "state") return true;
  const available = new Set(summary.available_commands || []);
  if (verb === "start") return available.has("start");
  if (verb === "end") return available.has("end");
  if (verb === "play") return available.has("play");
  if (verb === "choose") return available.has("choose");
  if (verb === "confirm") return available.has("confirm");
  if (verb === "proceed") return available.has("proceed");
  if (verb === "skip") return available.has("skip");
  if (verb === "leave") return available.has("leave");
  return available.has(verb);
}

function writeCommand(command) {
  if (fs.existsSync(commandPath)) return false;
  fs.writeFileSync(commandPath, `${command}\n`);
  log(`command: ${command}`);
  return true;
}

function choiceLabel(choice) {
  if (choice == null) return "";
  if (typeof choice === "string" || typeof choice === "number" || typeof choice === "boolean") {
    return String(choice);
  }
  for (const key of ["label", "name", "text", "symbol", "id"]) {
    if (choice[key] != null) return String(choice[key]);
  }
  return String(choice);
}

function choiceIndex(summary, patterns) {
  const choices = summary.choices || [];
  for (const pattern of patterns) {
    const index = choices.findIndex((choice) => pattern.test(choiceLabel(choice).toLowerCase()));
    if (index >= 0) return index;
  }
  return -1;
}

function livingMonsterIndex(combat) {
  const monsters = combat.monsters || [];
  let best = -1;
  let bestHp = Number.POSITIVE_INFINITY;
  for (const monster of monsters) {
    if (monster.gone || monster.hp <= 0) continue;
    if (monster.hp < bestHp) {
      best = monster.index;
      bestHp = monster.hp;
    }
  }
  return best < 0 ? 0 : best;
}

function cardScore(card, incomingDamage, playerHp = null) {
  const name = String(card.name || "").toLowerCase();
  if (!card.playable) return -1000;
  const hp = Number.isFinite(playerHp) ? playerHp : Number.POSITIVE_INFINITY;
  const defensive =
    name.includes("defend") ||
    name.includes("shrug") ||
    name.includes("true grit") ||
    name.includes("impervious") ||
    name.includes("power through") ||
    name.includes("flame barrier");
  if (incomingDamage > 0 && defensive && (incomingDamage >= 10 || incomingDamage >= hp / 4)) return 92;
  if (name.includes("immolate")) return 100;
  if (name.includes("carnage")) return 95;
  if (name.includes("bash")) return 90;
  if (name.includes("pommel")) return 88;
  if (name.includes("twin strike")) return 86;
  if (name.includes("thunderclap")) return 84;
  if (name.includes("cleave")) return 82;
  if (name.includes("strike")) return 70;
  if (name.includes("anger")) return 68;
  if (name.includes("inflame") || name.includes("metallicize") || name.includes("demon form")) return 65;
  if (incomingDamage > 0 && defensive) return 60;
  if (name.includes("battle trance") || name.includes("warcry")) return 50;
  if (name.includes("flex") || name.includes("spot weakness")) return 45;
  return card.type === "ATTACK" ? 40 : 10;
}

function incomingDamage(combat) {
  return (combat.monsters || []).reduce((sum, monster) => {
    if (monster.gone || monster.hp <= 0) return sum;
    const intent = String(monster.intent || "");
    const match = intent.match(/(\d+)/);
    return sum + (match ? Number.parseInt(match[1], 10) : 0);
  }, 0);
}

function combatCommand(summary) {
  const combat = summary.combat;
  const available = new Set(summary.available_commands || []);
  if (!combat) return "state";
  if (!available.has("play")) return available.has("end") ? "END" : "state";
  const target = livingMonsterIndex(combat);
  const incoming = incomingDamage(combat);
  const playerHp = summary.current_hp ?? combat.player_hp ?? null;
  const cards = (combat.hand || [])
    .filter((card) => card.playable)
    .sort((a, b) => cardScore(b, incoming, playerHp) - cardScore(a, incoming, playerHp));
  const card = cards[0];
  if (!card || cardScore(card, incoming, playerHp) < 0) return available.has("end") ? "END" : "state";
  return card.has_target ? `PLAY ${card.index} ${target}` : `PLAY ${card.index}`;
}

function rewardCommand(summary) {
  const available = new Set(summary.available_commands || []);
  if (!available.has("choose")) return available.has("proceed") ? "PROCEED" : "state";
  const relic = choiceIndex(summary, [/^relic$/, /relic/]);
  if (relic >= 0) return `CHOOSE ${relic}`;
  const gold = choiceIndex(summary, [/^gold$/, /stolen_gold/]);
  if (gold >= 0) return `CHOOSE ${gold}`;
  const card = choiceIndex(summary, [/^card$/]);
  if (card >= 0) return `CHOOSE ${card}`;
  const potion = choiceIndex(summary, [/^potion$/]);
  if (potion >= 0) {
    const potions = summary.potions || [];
    const hasEmptySlot =
      potions.length > 0 && potions.some((slot) => /potion slot/i.test(String(slot.name || "")));
    return hasEmptySlot ? `CHOOSE ${potion}` : available.has("proceed") ? "PROCEED" : "state";
  }
  return "PROCEED";
}

function cardRewardCommand(summary) {
  const available = new Set(summary.available_commands || []);
  if (!available.has("choose")) return available.has("skip") ? "SKIP" : "state";
  const priorities = [
    /immolate/,
    /offering/,
    /shrug/,
    /pommel/,
    /battle trance/,
    /thunderclap/,
    /cleave/,
    /anger/,
    /true grit/,
    /metallicize/,
    /clothesline/,
    /warcry/,
    /twin strike/,
    /headbutt/,
  ];
  const pick = choiceIndex(summary, priorities);
  return pick >= 0 ? `CHOOSE ${pick}` : "CHOOSE 0";
}

function mapCommand(summary) {
  const available = new Set(summary.available_commands || []);
  if (!available.has("choose")) return "state";
  const choices = summary.choices || [];
  if (choices.length === 0) return "state";
  let bestIndex = 0;
  let bestScore = Number.NEGATIVE_INFINITY;
  for (let index = 0; index < choices.length; index += 1) {
    const score = mapChoiceScore(choiceLabel(choices[index]));
    if (score > bestScore) {
      bestIndex = index;
      bestScore = score;
    }
  }
  return `CHOOSE ${bestIndex}`;
}

function mapChoiceScore(label) {
  const text = String(label || "").trim().toLowerCase();
  if (text === "e" || text.includes("elite")) return 100;
  if (text === "m" || text.startsWith("x=") || text.includes("monster")) return 80;
  if (text === "t" || text.includes("chest") || text.includes("treasure")) return 60;
  if (text === "?" || text.includes("event")) return 50;
  if (text === "$" || text.includes("shop")) return 30;
  if (text === "r" || text.includes("rest")) return 20;
  return 0;
}

function fallbackCommand(summary, attempted) {
  const available = new Set(summary.available_commands || []);
  const screen = String(summary.screen_type || "").toUpperCase();
  if (screen === "CARD_REWARD" && commandVerb(attempted) === "choose" && available.has("skip")) {
    return "SKIP";
  }
  if (screen === "COMBAT_REWARD" && available.has("proceed")) {
    return "PROCEED";
  }
  if (screen === "REST" && available.has("proceed")) {
    return "PROCEED";
  }
  if (screen === "SHOP_SCREEN" && available.has("leave")) {
    return "LEAVE";
  }
  return "state";
}

function nextCommand(summary) {
  const available = new Set(summary.available_commands || []);
  const screen = String(summary.screen_type || "").toUpperCase();
  if (summary.error) return "state";
  if (!summary.in_game && available.has("start")) {
    const seed = `${seedPrefix}${String(runIndex).padStart(4, "0")}`;
    runIndex += 1;
    saveRunIndex();
    return `START IRONCLAD 0 ${seed}`;
  }
  if (screen === "GAME_OVER") return available.has("proceed") ? "PROCEED" : "state";
  if (screen === "MAP") return mapCommand(summary);
  if (screen === "COMBAT_REWARD") return rewardCommand(summary);
  if (screen === "CARD_REWARD") return cardRewardCommand(summary);
  if (screen === "GRID") {
    if (available.has("confirm")) return "CONFIRM";
    return available.has("choose") && summary.choices?.length ? "CHOOSE 0" : "state";
  }
  if (screen === "SHOP_ROOM") {
    return available.has("choose") && summary.choices?.length ? "CHOOSE 0" : "state";
  }
  if (screen === "SHOP_SCREEN") {
    if (available.has("leave")) return "LEAVE";
    return available.has("choose") && summary.choices?.length ? "CHOOSE 0" : "state";
  }
  if (screen === "REST") {
    if (available.has("proceed")) return "PROCEED";
    const rest = choiceIndex(summary, [/rest/, /heal/]);
    return available.has("choose") && summary.choices?.length ? `CHOOSE ${rest >= 0 ? rest : 0}` : "state";
  }
  if (screen === "EVENT" || screen === "NONE" && summary.choices?.length) {
    return available.has("choose") ? "CHOOSE 0" : "state";
  }
  if (summary.combat) return combatCommand(summary);
  if (available.has("proceed")) return "PROCEED";
  if (available.has("confirm")) return "CONFIRM";
  if (available.has("choose") && summary.choices?.length) return "CHOOSE 0";
  return "state";
}

function stateSignature(summary) {
  return JSON.stringify({
    in_game: summary.in_game ?? false,
    screen_type: summary.screen_type ?? null,
    room_phase: summary.room_phase ?? null,
    room_type: summary.room_type ?? null,
    floor: summary.floor ?? null,
    hp: summary.current_hp ?? null,
    gold: summary.gold ?? null,
    available_commands: summary.available_commands ?? [],
    choices: summary.choices ?? [],
    combat_turn: summary.combat?.turn ?? null,
    combat_energy: summary.combat?.energy ?? null,
    combat_hand: summary.combat?.hand?.map((card) => [card.index, card.id, card.playable]) ?? null,
    monsters: summary.combat?.monsters?.map((monster) => [
      monster.index,
      monster.id,
      monster.hp,
      monster.block,
      monster.intent,
      monster.gone,
    ]) ?? null,
  });
}

function tick() {
  const staleReason = staleSessionReason();
  if (staleReason) {
    log(`stalled collector: ${staleReason}`);
    process.exit(2);
  }
  const summary = readJson(summaryPath);
  if (!summary) return;
  if (runIndex > maxRuns) {
    log(`max runs reached: ${maxRuns}`);
    process.exit(0);
  }
  if (!summary.ready_for_command) return;
  if (fs.existsSync(commandPath)) return;
  if (summary.step === lastStep) return;
  lastStep = summary.step;
  let command = nextCommand(summary);
  if (!commandIsAvailable(summary, command)) {
    const fallback = fallbackCommand(summary, command);
    log(`unavailable command: ${command}; fallback: ${fallback}`);
    command = fallback;
  }
  const signature = stateSignature(summary);
  const commandSignature = `${signature}\n${command}`;
  repeatedSameCommand = commandSignature === lastCommandSignature ? repeatedSameCommand + 1 : 1;
  if (repeatedSameCommand > maxSameCommand) {
    const fallback = fallbackCommand(summary, command);
    if (fallback !== command && commandIsAvailable(summary, fallback)) {
      log(`repeated command ${command} on unchanged state; fallback: ${fallback}`);
      command = fallback;
      repeatedSameCommand = 1;
      lastCommandSignature = `${signature}\n${command}`;
    } else {
      log(`stalled after repeating command ${command} ${repeatedSameCommand} times on step=${summary.step} screen=${summary.screen_type}`);
      process.exit(2);
    }
  } else {
    lastCommandSignature = commandSignature;
  }
  if (command.toLowerCase() === "state") {
    repeatedStatePolls = signature === lastSignature ? repeatedStatePolls + 1 : 1;
    if (repeatedStatePolls > maxStatePolls) {
      log(`stalled after ${repeatedStatePolls} state polls on step=${summary.step} screen=${summary.screen_type}`);
      process.exit(2);
    }
  } else {
    repeatedStatePolls = 0;
  }
  lastSignature = signature;
  writeCommand(command);
}

if (require.main === module) {
  log(`overnight collector started at ${root}`);
  log(`seed prefix=${seedPrefix} nextRun=${runIndex} maxRuns=${maxRuns} maxIdleMs=${maxIdleMs}`);
  setInterval(tick, tickMs);
  tick();
}

module.exports = {
  cardRewardCommand,
  cardScore,
  choiceLabel,
  choiceIndex,
  combatCommand,
  commandIsAvailable,
  commandVerb,
  fallbackCommand,
  mapCommand,
  mapChoiceScore,
  nextCommand,
  rewardCommand,
  staleSessionReasonFrom,
  stateSignature,
};
