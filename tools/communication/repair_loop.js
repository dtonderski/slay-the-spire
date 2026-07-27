#!/usr/bin/env node

/**
 * High-throughput collection supervisor.
 *
 * The live-trace agent owns one collection attempt. This process owns the
 * outer scheduling policy: archive each completed attempt, enqueue a repair
 * task on the first fidelity/mapping failure, and immediately ask that worker
 * for the next attempt. Repair work is deliberately decoupled from the game
 * workers and is never awaited by the collector.
 */

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const childProcess = require("child_process");
const readline = require("readline");

const repoRoot = path.resolve(__dirname, "..", "..");
const simulatorRoot = path.join(repoRoot, "simulator");
const DEFAULT_COLLECT_ARGS = [
  "--ascension",
  "0",
  "--victory",
  "--target-floor",
  "60",
  // Keep the collector responsive; callers can override this after `--`.
  "--combat-search-time-budget-ms",
  "250",
];
const CONTINUABLE_BLOCKERS = new Set([
  "completed_trace",
  "simulator_fidelity_break",
  "slaythedata_mapping_gap",
  "slaythedata_illegal_log",
  "slaythedata_incompatible_run",
  "run_ended_before_target",
]);

function usage() {
  return [
    "Usage:",
    "  node tools/communication/repair_loop.js run [options] -- [live-trace collect options]",
    "",
    "Options:",
    "  --live-trace PATH       live-trace executable (default: simulator/target/debug/live-trace)",
    "  --slaythedata-db PATH  SlayTheData database passed to live-trace",
    "  --bridge ID             bridge id; repeat once per worker",
    "  --bridge-session-dir PATH  CommunicationMod session dir; repeat once per worker",
    "  --workers N             concurrent game workers (default: 1)",
    "  --runs N                optional total attempt limit (default: unlimited)",
    "  --output-root PATH      immutable traces and repair queue root",
    "  --trace-root PATH       live-trace session trace root",
    "  --source-version ID    explicit collector build/version label",
    "  --repair-agent MODE     queue or codex (default: queue)",
    "  --repair-cwd PATH       isolated repair worktree for codex mode",
    "  --repair-workers N      must be 1 for the single isolated repair worktree",
    "  --boss-repair-agent MODE  queue or codex (default: codex)",
    "  --boss-repair-cwd PATH    active collector worktree for boss repairs",
    "  --fake                  use the live-trace fake bridge",
    "  --promote               allow successful attempts to promote traces",
    "  --help                  show this help",
    "",
    "Example:",
    "  node tools/communication/repair_loop.js run --bridge communication-mod --",
    "    --ascension 0 --victory --min-floor 51 --target-floor 60",
  ].join("\n");
}

function parseInteger(value, flag) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} requires a positive integer`);
  }
  return parsed;
}

function parseArgs(argv) {
  const options = {
    liveTrace: path.join(simulatorRoot, "target", "debug", "live-trace"),
    slaythedataDb: null,
    bridges: [],
    bridgeSessionDirs: [],
    workers: 1,
    runs: null,
    outputRoot: path.join(repoRoot, "live_traces_loop"),
    traceRoot: path.join(repoRoot, "live_traces_loop", "sessions"),
    sourceVersion: null,
    repairAgent: "queue",
    repairCwd: null,
    repairWorkers: 1,
    bossRepairAgent: "codex",
    bossRepairCwd: repoRoot,
    fake: false,
    promote: false,
    collectArgs: [...DEFAULT_COLLECT_ARGS],
  };
  let passThrough = false;
  const explicitCollectArgs = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (passThrough) {
      options.collectArgs.push(arg);
      continue;
    }
    if (arg === "--") {
      passThrough = true;
      options.collectArgs = [];
      continue;
    }
    const next = () => {
      index += 1;
      if (index >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[index];
    };
    switch (arg) {
      case "--live-trace":
        options.liveTrace = path.resolve(next());
        break;
      case "--slaythedata-db":
        options.slaythedataDb = path.resolve(next());
        break;
      case "--bridge":
        options.bridges.push(next());
        break;
      case "--bridge-session-dir":
        options.bridgeSessionDirs.push(path.resolve(next()));
        break;
      case "--workers":
        options.workers = parseInteger(next(), arg);
        break;
      case "--runs":
        options.runs = parseInteger(next(), arg);
        break;
      case "--output-root":
        options.outputRoot = path.resolve(next());
        break;
      case "--trace-root":
        options.traceRoot = path.resolve(next());
        break;
      case "--source-version":
        options.sourceVersion = next();
        break;
      case "--repair-agent":
        options.repairAgent = next();
        if (!new Set(["queue", "codex"]).has(options.repairAgent)) {
          throw new Error("--repair-agent must be queue or codex");
        }
        break;
      case "--repair-cwd":
        options.repairCwd = path.resolve(next());
        break;
      case "--repair-workers":
        options.repairWorkers = parseInteger(next(), arg);
        break;
      case "--boss-repair-agent":
        options.bossRepairAgent = next();
        if (!new Set(["queue", "codex"]).has(options.bossRepairAgent)) {
          throw new Error("--boss-repair-agent must be queue or codex");
        }
        break;
      case "--boss-repair-cwd":
        options.bossRepairCwd = path.resolve(next());
        break;
      case "--fake":
        options.fake = true;
        break;
      case "--promote":
        options.promote = true;
        break;
      case "--collect-arg":
        explicitCollectArgs.push(next());
        break;
      case "--help":
        options.help = true;
        break;
      default:
        throw new Error(`unknown option ${arg}\n\n${usage()}`);
    }
  }

  if (explicitCollectArgs.length > 0) {
    options.collectArgs = explicitCollectArgs;
  }
  if (options.collectArgs.length === 0) options.collectArgs = [...DEFAULT_COLLECT_ARGS];
  if (!options.collectArgs.includes("--combat-search-time-budget-ms")) {
    options.collectArgs.push("--combat-search-time-budget-ms", "250");
  }
  if (options.bridges.length > 0 && options.bridges.length !== options.workers) {
    throw new Error("provide exactly one --bridge per worker, or omit --bridge for one worker");
  }
  if (options.workers > 1 && options.bridges.length !== options.workers) {
    throw new Error("parallel workers require one explicit --bridge per worker");
  }
  if (options.bridgeSessionDirs.length > 0 && options.bridgeSessionDirs.length !== options.workers) {
    throw new Error("provide exactly one --bridge-session-dir per worker");
  }
  if (options.workers > 1 && options.bridgeSessionDirs.length !== options.workers) {
    throw new Error("parallel workers require one separate --bridge-session-dir per worker");
  }
  if (options.repairAgent === "codex" && !options.repairCwd) {
    throw new Error("--repair-agent codex requires --repair-cwd pointing at an isolated worktree");
  }
  if (options.repairAgent === "codex" && options.repairWorkers !== 1) {
    throw new Error("one --repair-cwd supports exactly one repair worker");
  }
  if (options.bossRepairAgent === "codex" && !options.bossRepairCwd) {
    throw new Error("--boss-repair-agent codex requires --boss-repair-cwd");
  }
  return options;
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const temporaryPath = `${filePath}.tmp-${process.pid}`;
  fs.writeFileSync(temporaryPath, `${JSON.stringify(value, null, 2)}\n`);
  fs.renameSync(temporaryPath, filePath);
}

function updateTask(filePath, fallback, mutate) {
  const task = readJson(filePath) || fallback;
  mutate(task);
  writeJson(filePath, task);
  return task;
}

function safeName(value) {
  return String(value || "unknown").replace(/[^A-Za-z0-9_.-]+/g, "_");
}

function fingerprintText(value) {
  return String(value || "")
    .toLowerCase()
    .replace(/[A-Za-z]:[\\/][^\s;,)]+/g, "<path>")
    .replace(/(session|run|step|floor|action)[ _-]*\d+/g, "$1#")
    .replace(/\b\d{3,}\b/g, "#")
    .replace(/\s+/g, " ")
    .trim();
}

function fingerprintFor(packet) {
  const source = [
    packet?.blocker_kind || "unknown",
    fingerprintText(packet?.first_simulator_diff_or_mapping_failure || "no-detail"),
  ].join("\n");
  return crypto.createHash("sha256").update(source).digest("hex").slice(0, 16);
}

function isUnsupportedBoss(result, packet) {
  const strict = result?.strict_verification || {};
  const detail = JSON.stringify({
    result: result || null,
    packet: packet || null,
    boundary: strict.first_boundary || null,
  }).toLowerCase();
  const unsupported = Number(strict.unsupported_actions || 0) > 0 || /\bunsupported\b/.test(detail);
  if (!unsupported) return false;

  const live = packet?.current_live_state_summary || {};
  const summary = live.summary || {};
  const room = [summary.room_type, live.room_phase, live.phase]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
  const floor = Number(live.floor ?? summary.floor ?? 0);
  const bossFloor = floor === 17 || floor === 34 || floor === 51;
  return /boss/.test(room) || (bossFloor && /combat/.test(room));
}

function taskDirectory(outputRoot, fingerprint) {
  return path.join(outputRoot, "tasks", fingerprint);
}

function repairPrompt(task) {
  const packet = task.repair_packet || {};
  const trace = task.archived_traces?.at(-1) || packet.trace_path || "(trace unavailable)";
  const bossRepair = task.repair_lane === "unsupported_boss";
  const worktreeText = bossRepair
    ? `Work in the active collector worktree. This repair is synchronous: the live game
session is paused at the unsupported boss and will be resumed after the supervisor
builds and verifies your change.`
    : "Work only in the isolated repair worktree supplied by the supervisor.";
  const collectorText = bossRepair
    ? `The supervisor will build live-trace, run focused parity, and resume the
same session. Do not edit the archived trace or queue metadata.`
    : `The collector is running independently. Do not modify its live session files,
the archived trace, or the repair queue metadata. A passing repair is marked
verified by the supervisor; promotion into the active collector build remains
an explicit operator action.`;
  const baselineText = bossRepair && task.workspace_test_baseline?.failures?.length
    ? `\n- Pre-existing workspace test failures (do not add any): ${task.workspace_test_baseline.failures.join(", ")}`
    : "";
  return `# Simulator repair task ${task.fingerprint}

This is an automated Slay the Spire fidelity repair task. ${worktreeText}
## Evidence

- Task: ${task.fingerprint}
- Blocker: ${packet.blocker_kind || "unknown"}
- Occurrences: ${task.occurrence_count}
- Archived trace: ${trace}
- Reproduction command: ${packet.reproduce_command || "not provided"}
- First difference or mapping failure: ${packet.first_simulator_diff_or_mapping_failure || "not provided"}
- Full packet: ${task.task_path}
${baselineText}

## Required behavior

1. Reproduce the failure from the trace and identify the first generic cause.
2. Fix the simulator, verifier, or command mapping that caused it.
3. Do not branch on the seed, run id, trace name, or observed state.
4. Never hydrate or repair authoritative simulator state from observations.
5. Add or update the smallest source-backed regression coverage.
6. Run the verification commands recorded in the task before reporting success.
7. Leave a concise final report describing the cause, files changed, and checks.

${collectorText}
`;
}

function archiveAttempt({ outputRoot, workerId, attemptId, packet, result, sourceVersion, repairLane = null }) {
  const packetValue = packet || {
    blocker_kind: result?.blocker_kind || "bridge_or_backend_error",
    first_simulator_diff_or_mapping_failure: result?.message || result?.reason || "missing repair packet",
    trace_path: null,
    session_id: result?.session_id || null,
  };
  const blocker = packetValue.blocker_kind || result?.blocker_kind || "bridge_or_backend_error";
  const fingerprint = fingerprintFor(packetValue);
  const observedAt = new Date().toISOString();
  const occurrenceId = [
    observedAt.replace(/[^0-9TZ]/g, ""),
    safeName(workerId),
    safeName(attemptId),
    crypto.randomBytes(4).toString("hex"),
  ].join("-");
  const traceSource = packetValue.trace_path && path.resolve(packetValue.trace_path);
  let archivedTracePath = null;
  if (traceSource && fs.existsSync(traceSource)) {
    const traceName = `${occurrenceId}-${safeName(packetValue.session_id)}.jsonl`;
    archivedTracePath = path.join(outputRoot, "traces", traceName);
    fs.mkdirSync(path.dirname(archivedTracePath), { recursive: true });
    fs.copyFileSync(traceSource, archivedTracePath, fs.constants.COPYFILE_EXCL);
  }

  const occurrence = {
    schema: 1,
    attempt_id: attemptId,
    worker_id: workerId,
    occurrence_id: occurrenceId,
    observed_at: observedAt,
    source_version: sourceVersion,
    result: result || null,
    packet: packetValue,
    archived_trace_path: archivedTracePath,
    repair_lane: repairLane,
  };
  if (blocker === "completed_trace" || blocker === "run_ended_before_target") {
    const completedPath = path.join(
      outputRoot,
      "completed",
      `${occurrenceId}.json`,
    );
    occurrence.packet = packetValue;
    occurrence.blocker_kind = blocker;
    writeJson(completedPath, occurrence);
    return {
      kind: blocker,
      fingerprint: null,
      task: null,
      occurrence,
      archivedTracePath,
    };
  }
  const directory = taskDirectory(outputRoot, fingerprint);
  const occurrencePath = path.join(directory, "occurrences", `${occurrenceId}.json`);
  writeJson(occurrencePath, occurrence);

  let task = readJson(path.join(directory, "task.json"));
  if (!task) {
    task = {
      schema: 1,
      type: "repair_task",
      fingerprint,
      status: "queued",
      created_at: occurrence.observed_at,
      occurrence_count: 0,
      source_versions: [],
      archived_traces: [],
      occurrence_paths: [],
      repair_packet: packetValue,
      verification_commands: verificationCommands(archivedTracePath),
      repair_lane: repairLane,
    };
  }
  if (repairLane && !task.repair_lane) task.repair_lane = repairLane;
  task.occurrence_count += 1;
  if (sourceVersion && !task.source_versions.includes(sourceVersion)) task.source_versions.push(sourceVersion);
  if (archivedTracePath && !task.archived_traces.includes(archivedTracePath)) {
    task.archived_traces.push(archivedTracePath);
  }
  task.occurrence_paths.push(occurrencePath);
  task.latest_occurrence_at = occurrence.observed_at;
  task.task_path = path.join(directory, "task.json");
  task.prompt_path = path.join(directory, "prompt.md");
  writeJson(task.task_path, task);
  if (!fs.existsSync(task.prompt_path)) fs.writeFileSync(task.prompt_path, repairPrompt(task));

  return {
    kind: "repair_task",
    fingerprint,
    task,
    occurrence,
    archivedTracePath,
  };
}

function verificationCommands(archivedTracePath) {
  const trace = archivedTracePath || "<archived-trace>";
  return [
    { cwd: "simulator", command: "cargo", args: ["fmt", "--check", "--all"] },
    {
      cwd: "simulator",
      command: "uv",
      args: ["run", "--python", "3.12", "cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
    },
    {
      cwd: "simulator",
      command: "uv",
      args: [
        "run",
        "--python",
        "3.12",
        "cargo",
        "run",
        "-p",
        "sts_verify",
        "--bin",
        "sts_verify",
        "--",
        "parity",
        trace,
      ],
    },
  ];
}

const WORKSPACE_TEST_ARGS = ["run", "--python", "3.12", "cargo", "test", "--workspace"];

function parseCargoTestFailures(output) {
  const failures = new Set();
  let inFailureList = false;
  for (const line of String(output || "").split(/\r?\n/)) {
    if (line.trim() === "failures:") {
      inFailureList = true;
      continue;
    }
    if (!inFailureList) continue;
    if (line.startsWith("    ") && line.trim()) {
      failures.add(line.trim());
      continue;
    }
    if (line.trim() && !line.startsWith(" ")) inFailureList = false;
  }
  return [...failures].sort();
}

async function workspaceTestSnapshot(repairCwd, outputPath) {
  const result = await runProcess(
    "uv",
    WORKSPACE_TEST_ARGS,
    path.join(repairCwd, "simulator"),
    outputPath,
  );
  const failures = parseCargoTestFailures(result.output);
  return {
    ok: result.code === 0,
    comparable: result.code === 0 || (result.code === 101 && failures.length > 0),
    code: result.code,
    signal: result.signal,
    failures,
  };
}

function compareWorkspaceTests(baseline, candidate) {
  if (!baseline.comparable || !candidate.comparable) {
    return {
      ok: false,
      reason: "workspace tests did not produce a comparable test-failure set",
      baseline,
      candidate,
    };
  }
  const baselineFailures = new Set(baseline.failures);
  const newFailures = candidate.failures.filter((failure) => !baselineFailures.has(failure));
  return {
    ok: candidate.ok || newFailures.length === 0,
    reason: newFailures.length === 0 ? null : "candidate introduced new workspace test failures",
    baseline_failures: baseline.failures,
    candidate_failures: candidate.failures,
    new_failures: newFailures,
  };
}

function bossVerificationCommands(archivedTracePath) {
  const trace = archivedTracePath || "<archived-trace>";
  return [
    { cwd: "simulator", command: "cargo", args: ["fmt", "--check", "--all"] },
    {
      cwd: "simulator",
      command: "uv",
      args: [
        "run",
        "--python",
        "3.12",
        "cargo",
        "run",
        "-p",
        "sts_verify",
        "--bin",
        "sts_verify",
        "--",
        "parity",
        trace,
      ],
    },
    {
      cwd: "simulator",
      command: "uv",
      args: [
        "run",
        "--python",
        "3.12",
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
      ],
    },
    {
      cwd: "simulator",
      command: "uv",
      args: ["run", "--python", "3.12", "cargo", "test", "--workspace"],
    },
    {
      cwd: "simulator",
      command: "uv",
      args: [
        "run",
        "--python",
        "3.12",
        "cargo",
        "build",
        "-p",
        "sts_live",
        "--bin",
        "live-trace",
      ],
    },
  ];
}

function shouldContinue(result, packet) {
  const blocker = result?.blocker_kind || packet?.blocker_kind;
  if (blocker === "bridge_or_backend_error") return false;
  if (result?.reason === "no_candidates" || result?.status === "no_candidates") return false;
  if (result?.status === "blocked") {
    if (result.reason === "game_over_before_target") return true;
    if (blocker !== "slaythedata_mapping_gap") return false;
    return !/(failed|search|database|bridge)/i.test(result.reason || result.message || "");
  }
  return CONTINUABLE_BLOCKERS.has(blocker);
}

function isInfrastructureFailure(result, packet) {
  return (result?.blocker_kind || packet?.blocker_kind) === "bridge_or_backend_error";
}

function buildWorkerArgs(options, workerId, packetPath, journalPath, bridgeId) {
  const runtimeArgs = [];
  if (options.slaythedataDb) runtimeArgs.push("--slaythedata-db", options.slaythedataDb);
  if (options.fake) runtimeArgs.push("--fake");
  const collectArgs = [...options.collectArgs, "--limit", "1", "--repair-packet", packetPath, "--journal", journalPath];
  if (bridgeId) collectArgs.push("--bridge", bridgeId);
  if (!options.promote) collectArgs.push("--no-promote");
  return [...runtimeArgs, "slaythedata", "agent", ...collectArgs];
}

function gitVersion() {
  if (process.env.STS_REPAIR_LOOP_SOURCE_VERSION) return process.env.STS_REPAIR_LOOP_SOURCE_VERSION;
  try {
    const gitEntry = path.join(repoRoot, ".git");
    const head = fs.readFileSync(path.join(gitEntry, "HEAD"), "utf8").trim();
    if (!head.startsWith("ref: ")) return head || "unknown";
    const ref = head.slice("ref: ".length);
    const refPath = path.join(gitEntry, ref);
    if (fs.existsSync(refPath)) return fs.readFileSync(refPath, "utf8").trim() || "unknown";
    const packedRefs = fs.existsSync(path.join(gitEntry, "packed-refs"))
      ? fs.readFileSync(path.join(gitEntry, "packed-refs"), "utf8")
      : "";
    const packed = packedRefs
      .split(/\r?\n/)
      .map((line) => line.trim().split(" "))
      .find(([hash, name]) => hash && name === ref);
    return packed?.[0] || "unknown";
  } catch {
    return "unknown";
  }
}

function runProcess(command, args, cwd, outputPath) {
  return new Promise((resolve) => {
    const child = childProcess.spawn(command, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
    const chunks = [];
    child.stdout.on("data", (chunk) => chunks.push(chunk));
    child.stderr.on("data", (chunk) => chunks.push(chunk));
    child.on("error", (error) => chunks.push(Buffer.from(`${error.stack || error}\n`)));
    child.on("exit", (code, signal) => {
      const output = Buffer.concat(chunks).toString("utf8");
      if (outputPath) fs.writeFileSync(outputPath, output);
      resolve({ code, signal, output });
    });
  });
}

function runCodexPrompt(prompt, repairCwd, logPath) {
  return new Promise((resolve) => {
    const child = childProcess.spawn(
      "codex",
      ["exec", "--json", "--sandbox", "workspace-write", "-C", repairCwd, "-"],
      { cwd: repairCwd, stdio: ["pipe", "pipe", "pipe"] },
    );
    const chunks = [];
    child.stdout.on("data", (chunk) => chunks.push(chunk));
    child.stderr.on("data", (chunk) => chunks.push(chunk));
    child.stdin.end(prompt);
    let settled = false;
    const finish = (exit) => {
      if (settled) return;
      settled = true;
      fs.writeFileSync(logPath, Buffer.concat(chunks).toString("utf8"));
      resolve(exit);
    };
    child.on("error", (error) => finish({ code: null, signal: null, error }));
    child.on("exit", (code, signal) => finish({ code, signal }));
  });
}

class RepairDispatcher {
  constructor(options) {
    this.options = options;
    this.active = 0;
    this.pending = [];
    this.dispatched = new Set();
  }

  enqueue(task) {
    if (this.options.repairAgent !== "codex") return;
    if (this.dispatched.has(task.fingerprint)) return;
    if (new Set(["repairing", "verifying", "verified"]).has(task.task?.status)) return;
    this.dispatched.add(task.fingerprint);
    this.pending.push(task);
    this.pump();
  }

  pump() {
    while (this.active < this.options.repairWorkers && this.pending.length > 0) {
      const task = this.pending.shift();
      this.active += 1;
      this.run(task)
        .catch((error) => console.error(`[repair ${task.fingerprint}] ${error.stack || error}`))
        .finally(() => {
          this.active -= 1;
          this.pump();
        });
    }
  }

  async run(task) {
    let taskValue = task.task;
    taskValue = updateTask(taskValue.task_path, taskValue, (latest) => {
      latest.status = "repairing";
      latest.repair_started_at = new Date().toISOString();
    });
    const taskDirectoryPath = path.dirname(taskValue.task_path);
    const logPath = path.join(taskDirectoryPath, "agent.log");
    const repairCwd = this.options.repairCwd;
    const prompt = fs.readFileSync(taskValue.prompt_path, "utf8");
    const exit = await runCodexPrompt(prompt, repairCwd, logPath);
    if (exit.code !== 0) {
      updateTask(taskValue.task_path, taskValue, (latest) => {
        latest.status = "repair_failed";
        latest.repair_exit = exit;
        latest.repair_finished_at = new Date().toISOString();
      });
      return;
    }

    taskValue = updateTask(taskValue.task_path, taskValue, (latest) => {
      latest.status = "verifying";
    });
    const verificationLogPath = path.join(taskDirectoryPath, "verification.log");
    const verification = await runVerification(taskValue, repairCwd, verificationLogPath);
    updateTask(taskValue.task_path, taskValue, (latest) => {
      latest.verification = verification;
      latest.status = verification.ok ? "verified" : "verification_failed";
      latest.repair_finished_at = new Date().toISOString();
    });
  }
}

class BossRepairDispatcher {
  constructor(options) {
    this.options = options;
    this.tail = Promise.resolve();
  }

  run(archive) {
    const operation = this.tail.then(() => this.runOne(archive));
    this.tail = operation.catch(() => {});
    return operation;
  }

  async runOne(archive) {
    let task = readJson(archive.task?.task_path) || archive.task;
    if (!task) return { ok: false, reason: "unsupported boss has no repair task" };
    if (
      task.status === "verified"
      && task.repair_lane === "unsupported_boss"
      && task.promoted_live_trace_path
      && fs.existsSync(task.promoted_live_trace_path)
    ) {
      return {
        ok: true,
        reason: null,
        repaired: false,
        liveTracePath: task.promoted_live_trace_path,
      };
    }
    if (this.options.bossRepairAgent !== "codex") {
      updateTask(task.task_path, task, (latest) => {
        latest.status = "awaiting_synchronous_repair";
        latest.repair_lane = "unsupported_boss";
      });
      return { ok: false, reason: "boss repair agent is queue-only" };
    }

    const repairCwd = this.options.bossRepairCwd;
    const taskRoot = path.dirname(task.task_path);
    task = updateTask(task.task_path, task, (latest) => {
      latest.status = "repairing";
      latest.repair_lane = "unsupported_boss";
      latest.repair_started_at = new Date().toISOString();
      latest.verification_commands = bossVerificationCommands(archive.archivedTracePath);
    });
    const baseline = await workspaceTestSnapshot(
      repairCwd,
      path.join(taskRoot, "boss-workspace-baseline.log"),
    );
    task = updateTask(task.task_path, task, (latest) => {
      latest.workspace_test_baseline = baseline;
    });
    if (!baseline.comparable) {
      updateTask(task.task_path, task, (latest) => {
        latest.status = "baseline_verification_failed";
        latest.repair_finished_at = new Date().toISOString();
      });
      return {
        ok: false,
        reason: "workspace baseline could not be measured before boss repair",
        baseline,
      };
    }
    fs.writeFileSync(task.prompt_path, repairPrompt(task));

    const exit = await runCodexPrompt(
      fs.readFileSync(task.prompt_path, "utf8"),
      repairCwd,
      path.join(taskRoot, "boss-agent.log"),
    );
    if (exit.code !== 0) {
      updateTask(task.task_path, task, (latest) => {
        latest.status = "repair_failed";
        latest.repair_exit = exit;
        latest.repair_finished_at = new Date().toISOString();
      });
      return { ok: false, reason: "boss repair agent failed", exit };
    }

    task = updateTask(task.task_path, task, (latest) => {
      latest.status = "verifying";
    });
    let verification = await runVerification(
      task,
      repairCwd,
      path.join(taskRoot, "boss-verification.log"),
    );
    if (verification.ok) {
      const candidate = await workspaceTestSnapshot(
        repairCwd,
        path.join(taskRoot, "boss-workspace-candidate.log"),
      );
      const workspaceRegression = compareWorkspaceTests(baseline, candidate);
      verification = {
        ...verification,
        ok: workspaceRegression.ok,
        workspace_regression: workspaceRegression,
      };
    }
    const liveTracePath = path.join(repairCwd, "simulator", "target", "debug", "live-trace");
    task = updateTask(task.task_path, task, (latest) => {
      latest.verification = verification;
      latest.status = verification.ok ? "verified" : "verification_failed";
      latest.repair_finished_at = new Date().toISOString();
      if (verification.ok && fs.existsSync(liveTracePath)) {
        latest.promoted_live_trace_path = liveTracePath;
      } else if (verification.ok) {
        latest.status = "verification_failed";
        latest.verification = {
          ...verification,
          ok: false,
          reason: `verified build did not produce ${liveTracePath}`,
        };
      }
    });
    return {
      ok: task.status === "verified",
      reason: task.status === "verified" ? null : "boss repair verification failed",
      verification: task.verification,
      repaired: task.status === "verified",
      liveTracePath: task.promoted_live_trace_path || null,
    };
  }
}

async function runVerification(task, repairCwd, outputPath) {
  const outputs = [];
  for (const command of task.verification_commands || []) {
    const cwd = command.cwd === "simulator" ? path.join(repairCwd, "simulator") : repairCwd;
    const result = await runProcess(command.command, command.args, cwd, null);
    outputs.push({ ...command, ...result });
    if (result.code !== 0) {
      fs.writeFileSync(outputPath, outputs.map(formatCommandOutput).join("\n"));
      return { ok: false, commands: outputs };
    }
  }
  fs.writeFileSync(outputPath, outputs.map(formatCommandOutput).join("\n"));
  return { ok: true, commands: outputs };
}

function formatCommandOutput(command) {
  return `$ ${command.command} ${command.args.join(" ")}\nexit=${command.code}\n${command.output || ""}`;
}

class Worker {
  constructor(controller, index, bridgeId, bridgeSessionDir) {
    this.controller = controller;
    this.index = index;
    this.id = `worker-${index + 1}`;
    this.bridgeId = bridgeId;
    this.bridgeSessionDir = bridgeSessionDir;
    this.packetPath = path.join(controller.options.outputRoot, "packets", `${this.id}.json`);
    this.journalPath = path.join(controller.options.outputRoot, "journals", `${this.id}.jsonl`);
    this.child = null;
    this.attemptId = null;
    this.command = null;
    this.started = false;
    this.repairing = false;
    this.resumeCount = 0;
    this.infrastructureFailures = 0;
    this.retryTimer = null;
    this.exitWaiters = [];
  }

  start() {
    const args = buildWorkerArgs(
      this.controller.options,
      this.id,
      this.packetPath,
      this.journalPath,
      this.bridgeId,
    );
    this.child = childProcess.spawn(this.controller.options.liveTrace, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        STS_LIVE_TRACE_ROOT: path.join(this.controller.options.traceRoot, this.id),
        ...(this.bridgeSessionDir
          ? { STS_LIVE_BRIDGE_SESSION_DIR: this.bridgeSessionDir }
          : {}),
      },
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.controller.log({
      event: "worker_started",
      worker: this.id,
      bridge: this.bridgeId,
      bridge_session_dir: this.bridgeSessionDir,
      args,
    });
    const lines = readline.createInterface({ input: this.child.stdout });
    lines.on("line", (line) => this.onLine(line));
    this.child.stderr.on("data", (chunk) => {
      this.controller.log({ event: "worker_stderr", worker: this.id, message: String(chunk).trim() });
    });
    this.child.on("error", (error) => this.controller.workerError(this, error));
    this.child.on("exit", (code, signal) => {
      for (const resolve of this.exitWaiters.splice(0)) resolve();
      this.controller.workerExit(this, code, signal);
    });
    this.started = true;
  }

  onLine(line) {
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      this.controller.log({ event: "worker_output", worker: this.id, line });
      return;
    }
    this.controller.log({ event: "agent_event", worker: this.id, payload: event });
    if (event.type === "command_started") this.command = event.command;
    if (event.type === "command_finished") this.controller.commandFinished(this, event);
  }

  send(command, requestId) {
    if (!this.child || this.child.exitCode !== null || this.child.stdin.destroyed) return false;
    this.command = command;
    this.child.stdin.write(`${JSON.stringify({ command, request_id: requestId })}\n`);
    return true;
  }

  stop() {
    if (!this.send("stop", `stop-${Date.now()}`) && this.child?.exitCode === null) {
      this.child.kill("SIGTERM");
    }
  }

  waitForExit() {
    if (!this.child || this.child.exitCode !== null) return Promise.resolve();
    return new Promise((resolve) => this.exitWaiters.push(resolve));
  }
}

class LoopController {
  constructor(options) {
    this.options = options;
    this.sourceVersion = options.sourceVersion || gitVersion();
    this.startedAttempts = 0;
    this.finishedAttempts = 0;
    this.nextAttemptId = 1;
    this.workers = [];
    this.dispatcher = new RepairDispatcher(options);
    this.bossDispatcher = new BossRepairDispatcher(options);
    this.logPath = path.join(options.outputRoot, "loop.jsonl");
    this.finishLogged = false;
    this.stopReason = null;
    fs.mkdirSync(options.outputRoot, { recursive: true });
    fs.mkdirSync(options.traceRoot, { recursive: true });
    this.logStream = fs.createWriteStream(this.logPath, { flags: "a" });
  }

  log(event) {
    const value = { at: new Date().toISOString(), ...event };
    const line = `${JSON.stringify(value)}\n`;
    this.logStream.write(line);
    if (event.event !== "agent_event") {
      process.stdout.write(line);
    }
  }

  start() {
    const workerCount = this.options.runs === null
      ? this.options.workers
      : Math.min(this.options.workers, this.options.runs);
    for (let index = 0; index < workerCount; index += 1) {
      const worker = new Worker(
        this,
        index,
        this.options.bridges[index] || null,
        this.options.bridgeSessionDirs[index] || null,
      );
      this.workers.push(worker);
      this.startAttempt(worker);
      worker.start();
    }
    this.log({
      event: "loop_started",
      run_limit: this.options.runs,
      indefinite: this.options.runs === null,
      workers: workerCount,
      source_version: this.sourceVersion,
    });
  }

  startAttempt(worker) {
    this.startedAttempts += 1;
    worker.attemptId = this.nextAttemptId;
    this.nextAttemptId += 1;
  }

  commandFinished(worker, event) {
    if (!new Set(["initial_collect", "next_run"]).has(event.command)) return;
    const result = event.result || {};
    const packet = readJson(worker.packetPath);
    const repairLane = isUnsupportedBoss(result, packet) ? "unsupported_boss" : null;
    const archive = archiveAttempt({
      outputRoot: this.options.outputRoot,
      workerId: worker.id,
      attemptId: worker.attemptId,
      packet,
      result,
      sourceVersion: this.sourceVersion,
      repairLane,
    });
    this.finishedAttempts += 1;
    const blocker = result.blocker_kind || packet?.blocker_kind || "unknown";
    this.log({
      event: "attempt_finished",
      worker: worker.id,
      attempt_id: worker.attemptId,
      status: result.status || null,
      blocker_kind: blocker,
      fingerprint: archive.kind === "repair_task" ? archive.fingerprint : null,
      archived_trace_path: archive.archivedTracePath,
    });
    if (isInfrastructureFailure(result, packet) && !this.runBudgetReached()) {
      this.scheduleInfrastructureRetry(worker, result);
      return;
    }
    worker.infrastructureFailures = 0;
    if (repairLane) {
      this.handleUnsupportedBoss(worker, archive, result, packet).catch((error) => {
        this.log({ event: "boss_repair_error", worker: worker.id, message: error.stack || String(error) });
        worker.repairing = false;
        this.stopReason = "unsupported_boss_repair_error";
        this.maybeFinish();
      });
      return;
    }
    if (archive.kind === "repair_task" && shouldContinue(result, packet)) {
      this.dispatcher.enqueue(archive);
    }

    const continuable = shouldContinue(result, packet);
    if (this.runBudgetReached() || !continuable) {
      if (!continuable) this.stopReason = `non_continuable_${result.status || result.reason || "result"}`;
      worker.stop();
      this.maybeFinish();
      return;
    }
    this.startAttempt(worker);
    worker.send("next_run", `attempt-${worker.attemptId}`);
  }

  async handleUnsupportedBoss(worker, archive, result, packet) {
    worker.repairing = true;
    this.log({
      event: "unsupported_boss_repair_started",
      worker: worker.id,
      attempt_id: worker.attemptId,
      fingerprint: archive.fingerprint,
      session_id: packet?.session_id || result?.session_id || null,
      mode: this.options.bossRepairAgent,
    });
    worker.stop();
    await worker.waitForExit();

    const repair = await this.bossDispatcher.run(archive);
    if (!repair.ok) {
      worker.repairing = false;
      this.stopReason = repair.reason || "unsupported_boss_repair_stopped";
      this.log({
        event: "unsupported_boss_repair_stopped",
        worker: worker.id,
        fingerprint: archive.fingerprint,
        reason: repair.reason,
      });
      this.maybeFinish();
      return;
    }
    if (repair.liveTracePath) {
      this.options.liveTrace = repair.liveTracePath;
    }
    if (repair.repaired) {
      this.sourceVersion = `${this.sourceVersion}+boss-${archive.fingerprint}`;
      this.log({
        event: "collector_build_promoted",
        worker: worker.id,
        fingerprint: archive.fingerprint,
        live_trace: this.options.liveTrace,
        source_version: this.sourceVersion,
      });
    }

    const sessionId = packet?.session_id || result?.session_id;
    const resumed = await this.resumeSession(worker, sessionId);
    if (!resumed.result) {
      worker.repairing = false;
      this.stopReason = "unsupported_boss_resume_failed";
      this.log({ event: "boss_resume_failed", worker: worker.id, output: resumed.output });
      this.maybeFinish();
      return;
    }
    worker.repairing = false;
    this.log({
      event: "unsupported_boss_repair_verified",
      worker: worker.id,
      fingerprint: archive.fingerprint,
      session_id: sessionId,
      resume_status: resumed.result.status || null,
    });
    this.handleResumedResult(worker, resumed.result, resumed.packet || packet);
  }

  async resumeSession(worker, sessionId) {
    if (!sessionId) return { result: null, packet: null, output: "unsupported boss has no session id" };
    const args = [];
    if (this.options.slaythedataDb) args.push("--slaythedata-db", this.options.slaythedataDb);
    if (this.options.fake) args.push("--fake");
    args.push(
      "slaythedata",
      "resume",
      sessionId,
      "--target-floor",
      findTargetFloor(this.options.collectArgs),
      "--combat-search-time-budget-ms",
      findCollectOption(this.options.collectArgs, "--combat-search-time-budget-ms", "250"),
      "--journal",
      worker.journalPath,
      "--no-promote",
    );
    const transitionBudget = findCollectOption(
      this.options.collectArgs,
      "--combat-search-transition-budget",
      null,
    );
    if (transitionBudget) {
      args.push("--combat-search-transition-budget", transitionBudget);
    }
    if (this.options.collectArgs.includes("--combat-search-dedup")) {
      args.push("--combat-search-dedup");
    }
    worker.resumeCount += 1;
    const outputPath = path.join(
      this.options.outputRoot,
      "resumes",
      `${worker.id}-${worker.resumeCount}.log`,
    );
    const resumed = await runJsonProcess(this.options.liveTrace, args, worker, outputPath);
    const result = resumed.value;
    const packet = result?.repair_packet || result?.attempts?.at(-1)?.repair_packet || null;
    return { ...resumed, result, packet };
  }

  handleResumedResult(worker, result, packet) {
    const attemptId = `${worker.attemptId}-resume-${worker.resumeCount}`;
    const repairLane = isUnsupportedBoss(result, packet) ? "unsupported_boss" : null;
    const archive = archiveAttempt({
      outputRoot: this.options.outputRoot,
      workerId: worker.id,
      attemptId,
      packet,
      result,
      sourceVersion: this.sourceVersion,
      repairLane,
    });
    this.finishedAttempts += 1;
    const blocker = result?.blocker_kind || packet?.blocker_kind || "unknown";
    this.log({
      event: "attempt_resumed_finished",
      worker: worker.id,
      attempt_id: attemptId,
      status: result?.status || null,
      blocker_kind: blocker,
      fingerprint: archive.kind === "repair_task" ? archive.fingerprint : null,
      archived_trace_path: archive.archivedTracePath,
    });
    if (repairLane) {
      this.handleUnsupportedBoss(worker, archive, result, packet).catch((error) => {
        this.log({ event: "boss_repair_error", worker: worker.id, message: error.stack || String(error) });
        worker.repairing = false;
        this.stopReason = "unsupported_boss_repair_error";
        this.maybeFinish();
      });
      return;
    }
    if (archive.kind === "repair_task" && shouldContinue(result, packet)) {
      this.dispatcher.enqueue(archive);
    }
    const continuable = shouldContinue(result, packet);
    if (this.runBudgetReached() || !continuable) {
      if (!continuable) this.stopReason = `non_continuable_${result.status || result.reason || "result"}`;
      this.maybeFinish();
      return;
    }
    this.startAttempt(worker);
    worker.start();
  }

  workerError(worker, error) {
    this.stopReason = "worker_error";
    this.log({ event: "worker_error", worker: worker.id, message: error.stack || error });
  }

  workerExit(worker, code, signal) {
    this.log({ event: "worker_exit", worker: worker.id, code, signal });
    this.maybeFinish();
  }

  maybeFinish() {
    if (this.workers.some((worker) => worker.repairing)) return;
    if (this.workers.some((worker) => worker.retryTimer !== null)) return;
    if (this.workers.some((worker) => worker.child && worker.child.exitCode === null)) return;
    if (this.finishLogged) return;
    this.finishLogged = true;
    const budgetReached = this.runBudgetReached() && !this.stopReason;
    this.log({
      event: budgetReached ? "loop_finished" : "loop_stopped",
      reason: budgetReached ? "run_budget_reached" : this.stopReason || "no_worker_can_continue",
      started: this.startedAttempts,
      finished: this.finishedAttempts,
    });
    this.logStream.end();
  }

  runBudgetReached() {
    return this.options.runs !== null && this.startedAttempts >= this.options.runs;
  }

  scheduleInfrastructureRetry(worker, result) {
    worker.infrastructureFailures += 1;
    const delayMs = Math.min(30_000, 1_000 * (2 ** Math.min(worker.infrastructureFailures - 1, 5)));
    this.log({
      event: "worker_retry_scheduled",
      worker: worker.id,
      delay_ms: delayMs,
      consecutive_failures: worker.infrastructureFailures,
      reason: result.reason || result.message || "bridge_or_backend_error",
    });
    worker.retryTimer = setTimeout(() => {
      worker.retryTimer = null;
      if (!worker.child || worker.child.exitCode !== null) {
        this.stopReason = "worker_exited_before_retry";
        this.maybeFinish();
        return;
      }
      this.startAttempt(worker);
      if (!worker.send("next_run", `attempt-${worker.attemptId}`)) {
        this.stopReason = "worker_retry_send_failed";
        worker.stop();
        this.maybeFinish();
      }
    }, delayMs);
  }

  stop() {
    this.stopReason = this.stopReason || "operator_shutdown";
    for (const worker of this.workers) {
      if (worker.retryTimer !== null) {
        clearTimeout(worker.retryTimer);
        worker.retryTimer = null;
      }
      worker.stop();
    }
  }
}

function findTargetFloor(collectArgs) {
  return findCollectOption(collectArgs, "--target-floor", "60");
}

function findCollectOption(collectArgs, flag, fallback) {
  const index = collectArgs.lastIndexOf(flag);
  return index >= 0 && collectArgs[index + 1] ? collectArgs[index + 1] : fallback;
}

function runJsonProcess(command, args, worker, outputPath) {
  return new Promise((resolve) => {
    const child = childProcess.spawn(command, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        STS_LIVE_TRACE_ROOT: path.join(worker.controller.options.traceRoot, worker.id),
        ...(worker.bridgeSessionDir
          ? { STS_LIVE_BRIDGE_SESSION_DIR: worker.bridgeSessionDir }
          : {}),
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let settled = false;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      resolve(value);
    };
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", (error) => {
      const output = `${Buffer.concat(stdout)}${Buffer.concat(stderr)}\n${error.stack || error}`;
      fs.mkdirSync(path.dirname(outputPath), { recursive: true });
      fs.writeFileSync(outputPath, output);
      finish({ value: null, output, code: null, signal: null });
    });
    child.on("exit", (code, signal) => {
      const stdoutText = Buffer.concat(stdout).toString("utf8");
      const stderrText = Buffer.concat(stderr).toString("utf8");
      const output = `${stdoutText}${stderrText}`;
      fs.mkdirSync(path.dirname(outputPath), { recursive: true });
      fs.writeFileSync(outputPath, output);
      let value = null;
      for (const line of stdoutText.trim().split(/\r?\n/).reverse()) {
        try {
          const candidate = JSON.parse(line);
          if (candidate && typeof candidate === "object") {
            value = candidate;
            break;
          }
        } catch {
          // Progress lines are not the final CLI result.
        }
      }
      if (code !== 0 && !value) {
        value = { status: "blocked", reason: "resume_process_failed", message: output };
      }
      finish({ value, output, code, signal });
    });
  });
}

function main(argv = process.argv.slice(2)) {
  if (argv[0] !== "run") {
    console.error(usage());
    process.exitCode = 2;
    return;
  }
  let options;
  try {
    options = parseArgs(argv.slice(1));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 2;
    return;
  }
  if (options.help) {
    console.log(usage());
    return;
  }
  if (!fs.existsSync(options.liveTrace)) {
    console.error(`live-trace executable not found: ${options.liveTrace}`);
    process.exitCode = 2;
    return;
  }
  if (options.repairAgent === "codex" && !fs.existsSync(options.repairCwd)) {
    console.error(`repair worktree not found: ${options.repairCwd}`);
    process.exitCode = 2;
    return;
  }
  if (options.bossRepairAgent === "codex" && !fs.existsSync(options.bossRepairCwd)) {
    console.error(`boss repair worktree not found: ${options.bossRepairCwd}`);
    process.exitCode = 2;
    return;
  }
  if (!process.env.UV_CACHE_DIR) {
    process.env.UV_CACHE_DIR = path.join(options.outputRoot, "cache", "uv");
  }
  const controller = new LoopController(options);
  process.on("SIGINT", () => {
    controller.log({ event: "shutdown_requested" });
    controller.stop();
  });
  controller.start();
}

if (require.main === module) main();

module.exports = {
  CONTINUABLE_BLOCKERS,
  DEFAULT_COLLECT_ARGS,
  archiveAttempt,
  buildWorkerArgs,
  compareWorkspaceTests,
  fingerprintFor,
  fingerprintText,
  isUnsupportedBoss,
  isInfrastructureFailure,
  parseCargoTestFailures,
  parseArgs,
  repairPrompt,
  shouldContinue,
  verificationCommands,
};
