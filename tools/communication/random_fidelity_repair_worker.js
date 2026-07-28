#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");
const {
  candidateChanges,
  prepareWorkspace,
  snapshotFiles,
} = require("./random_fidelity_repair_workspace");

const root = path.resolve(__dirname, "..", "..");
const queueScript = path.join(__dirname, "random_fidelity_repair_queue.js");

function positiveInteger(value, name) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}

function retryDelayMs(value = process.env.STS_RANDOM_REPAIR_RETRY_MS || "5000") {
  return positiveInteger(value, "STS_RANDOM_REPAIR_RETRY_MS");
}

function repairCodexCommand(environment = process.env) {
  return environment.STS_RANDOM_REPAIR_CODEX ||
    environment.STS_RANDOM_REPAIR_CODEX_BIN ||
    "codex";
}

function repairProcessEnvironment(environment = process.env, nodePath = process.execPath) {
  return {
    ...environment,
    PATH: [
      path.dirname(nodePath),
      environment.PATH,
    ].filter(Boolean).join(path.delimiter),
  };
}

function workerIdentity(index) {
  return `overnight-luna-${index + 1}`;
}

function codexArgs({
  cwd,
  model = process.env.STS_RANDOM_REPAIR_MODEL || "gpt-5.6-luna",
  effort = process.env.STS_RANDOM_REPAIR_EFFORT || "xhigh",
}) {
  return [
    "exec",
    "--json",
    "--ephemeral",
    "--skip-git-repo-check",
    "--sandbox",
    "workspace-write",
    "-C",
    cwd,
    "-m",
    model,
    "-c",
    `model_reasoning_effort="${effort}"`,
    "-c",
    'service_tier="priority"',
    "-c",
    'approval_policy="never"',
    "-",
  ];
}

function parseClaim(stdout, exitCode) {
  if (!stdout.trim() && exitCode === 2) return null;
  const parsed = JSON.parse(stdout);
  return parsed.task || null;
}

function repairPrompt(task, worker) {
  const occurrence = task.occurrences?.[0] || {};
  return `Repair random-fidelity divergence ${task.fingerprint}.

You are repair worker ${worker}. You are in an isolated candidate workspace;
the live verifier tree is read-only and must not be modified. Do not spawn
subagents. Read AGENT_RULES.md and docs/research.md before making changes.
Diagnose the first divergence using the claimed minimized and full traces.
Implement the smallest generic simulator fix. Never add seed-specific behavior,
hydrate simulator state from observations, weaken comparisons, or modify
unrelated mechanics. Do not run Git state-changing commands.

Fingerprint: ${task.fingerprint}
Full trace: ${occurrence.trace || "see task.json"}
Minimized trace: ${occurrence.minimized_trace || task.minimized_trace || "see task.json"}

Add focused regression coverage where appropriate and run focused tests against
this workspace. Do not run the repair queue recheck or broad corpus gate; the
serialized integration lane performs both against a clean staging tree. If you
cannot safely repair it, make no speculative change and explain why.`;
}

function writeJsonAtomic(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const temporary = `${filePath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
  fs.renameSync(temporary, filePath);
}

function runCaptured(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: options.cwd || root,
      env: options.env || process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.once("error", (error) => resolve({
      code: null,
      signal: null,
      stdout: Buffer.concat(stdout).toString("utf8"),
      stderr: `${Buffer.concat(stderr).toString("utf8")}${error.stack || error}\n`,
    }));
    child.once("exit", (code, signal) => resolve({
      code,
      signal,
      stdout: Buffer.concat(stdout).toString("utf8"),
      stderr: Buffer.concat(stderr).toString("utf8"),
    }));
  });
}

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function taskState(outputDir, fingerprint) {
  const taskPath = path.join(outputDir, "repair_tasks", fingerprint, "task.json");
  try {
    return JSON.parse(fs.readFileSync(taskPath, "utf8"));
  } catch {
    return null;
  }
}

function candidateState(outputDir, fingerprint) {
  try {
    return JSON.parse(
      fs.readFileSync(
        path.join(outputDir, "repair_candidates", fingerprint, "candidate.json"),
        "utf8",
      ),
    );
  } catch {
    return null;
  }
}

function claimedFingerprintsForWorker(outputDir, worker) {
  const tasksDir = path.join(outputDir, "repair_tasks");
  try {
    return fs.readdirSync(tasksDir)
      .flatMap((fingerprint) => {
        const task = taskState(outputDir, fingerprint);
        return task?.status === "in_progress" && task?.repair?.worker === worker
          ? [fingerprint]
          : [];
      })
      .sort();
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

async function releaseClaim(outputDir, task, worker, note) {
  const current = taskState(outputDir, task.fingerprint);
  if (current?.status !== "in_progress" || current?.repair?.worker !== worker) return;
  await runCaptured(
    process.execPath,
    [queueScript, "release", task.fingerprint, worker, note],
    {
      env: {
        ...process.env,
        STS_RANDOM_OUTPUT_DIR: outputDir,
      },
    },
  );
}

async function recoverClaims(outputDir, worker) {
  for (const fingerprint of claimedFingerprintsForWorker(outputDir, worker)) {
    const candidate = candidateState(outputDir, fingerprint);
    if (candidate && new Set(["queued", "gating"]).has(candidate.status)) continue;
    await releaseClaim(
      outputDir,
      { fingerprint },
      worker,
      "repair lane restarted before completing recheck",
    );
  }
}

async function runCodex(task, worker, outputDir, logDir, workspaceCwd = root, targetDir = null) {
  const stamp = new Date().toISOString().replaceAll(":", "-");
  const logPath = path.join(logDir, `${stamp}-${task.fingerprint}.jsonl`);
  fs.mkdirSync(logDir, { recursive: true });
  return new Promise((resolve) => {
    const log = fs.createWriteStream(logPath, { flags: "wx" });
    const child = spawn(
      repairCodexCommand(),
      codexArgs({ cwd: workspaceCwd }),
      {
        cwd: workspaceCwd,
        env: {
          ...repairProcessEnvironment(),
          STS_RANDOM_OUTPUT_DIR: outputDir,
          ...(targetDir ? { CARGO_TARGET_DIR: targetDir } : {}),
        },
        stdio: ["pipe", "pipe", "pipe"],
      },
    );
    child.stdout.pipe(log, { end: false });
    child.stderr.pipe(log, { end: false });
    child.stdin.end(repairPrompt(task, worker));
    let settled = false;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      log.end(() => resolve({ ...result, log: logPath }));
    };
    child.once("error", (error) => finish({ code: null, signal: null, error: String(error) }));
    child.once("exit", (code, signal) => finish({ code, signal }));
  });
}

function writeCandidate(outputDir, candidate) {
  const candidateDir = path.join(outputDir, "repair_candidates", candidate.fingerprint);
  fs.mkdirSync(candidateDir, { recursive: true });
  writeJsonAtomic(path.join(candidateDir, "candidate.json"), candidate);
}

async function waitForCandidate(outputDir, fingerprint, statusPath, worker, delay) {
  for (;;) {
    const candidate = candidateState(outputDir, fingerprint);
    if (!candidate || !new Set(["queued", "gating"]).has(candidate.status)) return candidate;
    writeJsonAtomic(statusPath, {
      worker,
      state: "awaiting_gate",
      fingerprint,
      candidate_status: candidate.status,
      updated_at: new Date().toISOString(),
    });
    await sleep(delay);
  }
}

async function main() {
  const workerCount = positiveInteger(
    process.env.STS_RANDOM_REPAIR_WORKERS || "8",
    "STS_RANDOM_REPAIR_WORKERS",
  );
  const index = Number.parseInt(process.env.STS_RANDOM_REPAIR_WORKER_INDEX || "0", 10);
  if (!Number.isInteger(index) || index < 0 || index >= workerCount) {
    throw new Error("STS_RANDOM_REPAIR_WORKER_INDEX must identify an existing worker");
  }
  const outputDir = path.resolve(
    process.env.STS_RANDOM_OUTPUT_DIR || path.join(root, "random_traces_loop"),
  );
  const worker = workerIdentity(index);
  const statusPath = path.join(outputDir, "repair_worker_status", `${worker}.json`);
  const logDir = path.join(outputDir, "repair_agent_logs", worker);
  const delay = retryDelayMs();
  await recoverClaims(outputDir, worker);
  for (const fingerprint of claimedFingerprintsForWorker(outputDir, worker)) {
    await waitForCandidate(outputDir, fingerprint, statusPath, worker, delay);
  }

  for (;;) {
    writeJsonAtomic(statusPath, {
      worker,
      state: "claiming",
      updated_at: new Date().toISOString(),
    });
    const claimResult = await runCaptured(
      process.execPath,
      [queueScript, "claim-ready", worker],
      {
        env: {
          ...process.env,
          STS_RANDOM_OUTPUT_DIR: outputDir,
        },
      },
    );
    if (claimResult.code !== 0 && claimResult.code !== 2) {
      writeJsonAtomic(statusPath, {
        worker,
        state: "claim_error",
        updated_at: new Date().toISOString(),
        error: claimResult.stderr.trim() || claimResult.stdout.trim(),
      });
      await sleep(delay);
      continue;
    }

    let task;
    try {
      task = parseClaim(claimResult.stdout, claimResult.code);
    } catch (error) {
      writeJsonAtomic(statusPath, {
        worker,
        state: "claim_error",
        updated_at: new Date().toISOString(),
        error: `invalid claim response: ${error.message}`,
      });
      await sleep(delay);
      continue;
    }
    if (!task) {
      writeJsonAtomic(statusPath, {
        worker,
        state: "idle",
        updated_at: new Date().toISOString(),
      });
      await sleep(delay);
      continue;
    }

    writeJsonAtomic(statusPath, {
      worker,
      state: "preparing_workspace",
      fingerprint: task.fingerprint,
      updated_at: new Date().toISOString(),
    });
    const workspaceRoot = path.join(
      process.env.STS_RANDOM_REPAIR_WORKSPACE_ROOT || "/tmp/sts-random-repair-workspaces",
      worker,
      task.fingerprint,
    );
    const workspace = await prepareWorkspace({
      sourceRoot: root,
      workspaceRoot,
      corpusRoot: path.join(root, "simulator", "verification", "corpus"),
    });
    writeJsonAtomic(statusPath, {
      worker,
      state: "repairing_isolated",
      fingerprint: task.fingerprint,
      workspace: workspace.work,
      updated_at: new Date().toISOString(),
    });
    const targetDir = path.join(
      process.env.STS_RANDOM_REPAIR_TARGET_ROOT || "/tmp/sts-random-repair-targets",
      worker,
    );
    const result = await runCodex(
      task,
      worker,
      outputDir,
      logDir,
      workspace.work,
      targetDir,
    );
    if (result.code !== 0) {
      const errorDetail = result.error ? ` error=${result.error}` : "";
      await releaseClaim(
        outputDir,
        task,
        worker,
        `isolated Codex exited code=${result.code} signal=${result.signal}${errorDetail}`,
      );
      fs.rmSync(workspaceRoot, { recursive: true, force: true });
      continue;
    }
    const changes = candidateChanges(
      workspace.baseline_files,
      snapshotFiles(workspace.work),
    );
    if (changes.length === 0) {
      await releaseClaim(outputDir, task, worker, "isolated repair produced no candidate changes");
      fs.rmSync(workspaceRoot, { recursive: true, force: true });
      continue;
    }
    writeCandidate(outputDir, {
      schema: 1,
      fingerprint: task.fingerprint,
      worker,
      status: "queued",
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      baseline: workspace.baseline,
      work: workspace.work,
      changes,
      agent_log: result.log,
    });
    const disposition = await waitForCandidate(
      outputDir,
      task.fingerprint,
      statusPath,
      worker,
      delay,
    );
    const current = taskState(outputDir, task.fingerprint);
    writeJsonAtomic(statusPath, {
      worker,
      state: "completed_attempt",
      fingerprint: task.fingerprint,
      task_status: current?.status || null,
      candidate_status: disposition?.status || null,
      updated_at: new Date().toISOString(),
      codex_exit_code: result.code,
      codex_signal: result.signal,
      log: result.log,
    });
    fs.rmSync(workspaceRoot, { recursive: true, force: true });
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exit(1);
  });
}

module.exports = {
  claimedFingerprintsForWorker,
  codexArgs,
  parseClaim,
  repairCodexCommand,
  repairProcessEnvironment,
  repairPrompt,
  retryDelayMs,
  workerIdentity,
};
