#!/usr/bin/env node

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const {
  applyCandidateFiles,
  prepareWorkspace,
} = require("./random_fidelity_repair_workspace");

const root = path.resolve(__dirname, "..", "..");

function writeJsonAtomic(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const temporary = `${filePath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
  fs.renameSync(temporary, filePath);
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function readCandidates(outputDir) {
  const directory = path.join(outputDir, "repair_candidates");
  try {
    return fs.readdirSync(directory).flatMap((fingerprint) => {
      const candidate = readJson(path.join(directory, fingerprint, "candidate.json"));
      return candidate ? [candidate] : [];
    });
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

function nextCandidate(candidates) {
  return [...candidates]
    .filter((candidate) => candidate.status === "queued")
    .sort((left, right) =>
      String(left.created_at).localeCompare(String(right.created_at)))[0] || null;
}

function recoverGatingCandidates(outputDir) {
  const recovered = [];
  for (const candidate of readCandidates(outputDir)) {
    if (candidate.status !== "gating") continue;
    updateCandidate(outputDir, candidate, {
      status: "queued",
      recovery_note: "integrator restarted while candidate was gating",
    });
    recovered.push(candidate.fingerprint);
  }
  return recovered;
}

function focusedDisposition(fingerprint, verification) {
  if (verification.fingerprint === fingerprint) {
    return { accepted: false, note: `focused replay still fails with ${fingerprint}` };
  }
  return {
    accepted: true,
    note: verification.fingerprint
      ? `focused replay advanced to ${verification.fingerprint}`
      : "focused replay reached strict parity",
  };
}

function gateHasNoNewFailures(baselineFailures, candidateFailures) {
  const baseline = new Set(baselineFailures);
  return candidateFailures.every((trace) => baseline.has(trace));
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

function verifierBinaryPath(targetDir) {
  return path.join(
    targetDir,
    "debug",
    process.platform === "win32" ? "sts_verify.exe" : "sts_verify",
  );
}

function uvCommand(environment = process.env) {
  if (environment.STS_RANDOM_UV_BIN) return environment.STS_RANDOM_UV_BIN;
  const userUv = path.join(os.homedir(), ".local", "bin", "uv");
  return fs.existsSync(userUv) ? userUv : "uv";
}

async function buildVerifier(stageRoot, targetDir) {
  const result = await runCaptured(
    uvCommand(),
    ["run", "--python", "3.12", "cargo", "build", "-p", "sts_verify", "--bin", "sts_verify"],
    {
      cwd: path.join(stageRoot, "simulator"),
      env: {
        ...process.env,
        CARGO_TARGET_DIR: targetDir,
        UV_CACHE_DIR: process.env.UV_CACHE_DIR || "/tmp/sts-uv-cache",
      },
    },
  );
  if (result.code !== 0) {
    throw new Error(`isolated verifier build failed: ${result.stderr || result.stdout}`);
  }
  return verifierBinaryPath(targetDir);
}

function expectedHashes(candidate) {
  return Object.fromEntries(
    candidate.changes.map((change) => [change.path, change.baseline_sha256]),
  );
}

function snapshotPermanentCorpus(sourceCorpus, destinationCorpus) {
  fs.rmSync(destinationCorpus, { recursive: true, force: true });
  fs.mkdirSync(path.join(destinationCorpus, "permanent_traces"), { recursive: true });
  fs.copyFileSync(
    path.join(sourceCorpus, "permanent_traces.json"),
    path.join(destinationCorpus, "permanent_traces.json"),
  );
  for (const name of fs.readdirSync(path.join(sourceCorpus, "permanent_traces"))) {
    fs.symlinkSync(
      path.join(sourceCorpus, "permanent_traces", name),
      path.join(destinationCorpus, "permanent_traces", name),
    );
  }
}

function retainResolvedPrefix(stageRoot, fingerprint) {
  const manifestPath = path.join(
    stageRoot,
    "simulator",
    "verification",
    "corpus",
    "permanent_traces.json",
  );
  const traceName = `random-fidelity-${fingerprint}.jsonl`;
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const entry = manifest.entries.find((value) => value.trace === traceName);
  if (!entry) throw new Error(`staging manifest has no ${traceName}`);
  const traceText = fs.readFileSync(
    path.join(stageRoot, "simulator", "verification", "corpus", "permanent_traces", traceName),
    "utf8",
  );
  const { expectationForTask } = require(
    path.join(stageRoot, "tools", "communication", "random_fidelity_corpus_promoter.js"),
  );
  entry.expectation = expectationForTask(
    { fingerprint, status: "resolved" },
    traceText,
  );
  writeJsonAtomic(manifestPath, manifest);
}

async function verifyFocused({ stageRoot, task, stageOutput, targetDir }) {
  const occurrence = task.occurrences?.[0];
  if (!occurrence?.trace) throw new Error(`task ${task.fingerprint} has no replayable trace`);
  const script = [
    "const {verifyEntry}=require(process.argv[1]);",
    "const result=verifyEntry({trace:process.argv[2]},{acceptVerifiedPrefix:true});",
    "process.stdout.write(JSON.stringify(result));",
  ].join("");
  const result = await runCaptured(
    process.execPath,
    [
      "-e",
      script,
      path.join(stageRoot, "tools", "communication", "random_fidelity_verifier.js"),
      occurrence.trace,
    ],
    {
      cwd: stageRoot,
      env: {
        ...process.env,
        STS_RANDOM_OUTPUT_DIR: stageOutput,
        CARGO_TARGET_DIR: targetDir,
        STS_VERIFY_BIN: verifierBinaryPath(targetDir),
      },
    },
  );
  if (result.code !== 0) {
    throw new Error(`focused verifier failed: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

async function runBroadGate({ stageRoot, stageOutput, targetDir, logPath }) {
  const result = await runCaptured(
    process.execPath,
    [path.join(stageRoot, "tools", "communication", "random_fidelity_batch_gate.js")],
    {
      cwd: stageRoot,
      env: {
        ...process.env,
        STS_RANDOM_OUTPUT_DIR: stageOutput,
        CARGO_TARGET_DIR: targetDir,
        STS_VERIFY_BIN: verifierBinaryPath(targetDir),
      },
    },
  );
  const fullLogPath = path.join(stageOutput, "promotion_gate_latest.log");
  const fullOutput = fs.existsSync(fullLogPath)
    ? fs.readFileSync(fullLogPath, "utf8")
    : `${result.stdout}\n${result.stderr}`;
  fs.writeFileSync(logPath, fullOutput);
  return { ...result, fullOutput };
}

async function queueCommand(outputDir, ...args) {
  return runCaptured(
    process.execPath,
    [path.join(root, "tools", "communication", "random_fidelity_repair_queue.js"), ...args],
    {
      env: {
        ...process.env,
        STS_RANDOM_OUTPUT_DIR: outputDir,
      },
    },
  );
}

function updateCandidate(outputDir, candidate, updates) {
  const updated = {
    ...candidate,
    ...updates,
    updated_at: new Date().toISOString(),
  };
  writeJsonAtomic(
    path.join(outputDir, "repair_candidates", candidate.fingerprint, "candidate.json"),
    updated,
  );
  return updated;
}

async function rejectCandidate(outputDir, candidate, reason) {
  await queueCommand(outputDir, "release", candidate.fingerprint, candidate.worker, reason);
  updateCandidate(outputDir, candidate, { status: "rejected", reason });
}

async function integrateCandidate(outputDir, candidate) {
  const taskPath = path.join(
    outputDir,
    "repair_tasks",
    candidate.fingerprint,
    "task.json",
  );
  const task = readJson(taskPath);
  if (
    !task ||
    task.status !== "in_progress" ||
    task.repair?.worker !== candidate.worker
  ) {
    updateCandidate(outputDir, candidate, {
      status: "rejected",
      reason: "candidate no longer owns its repair task",
    });
    return;
  }
  candidate = updateCandidate(outputDir, candidate, { status: "gating" });
  const stageRoot = path.join(outputDir, "repair_integration", "workspace");
  const stageOutput = path.join(outputDir, "repair_integration", "gate_output");
  const targetDir = path.join(outputDir, "repair_integration", "target");
  const baselinePath = path.join(outputDir, "repair_integration", "baseline_failures.json");
  const logPath = path.join(
    outputDir,
    "repair_candidates",
    candidate.fingerprint,
    "gate.log",
  );
  try {
    const stage = await prepareWorkspace({
      sourceRoot: root,
      workspaceRoot: stageRoot,
      corpusRoot: path.join(root, "simulator", "verification", "corpus"),
    });
    const corpusPath = path.join(stage.work, "simulator", "verification", "corpus");
    fs.unlinkSync(corpusPath);
    snapshotPermanentCorpus(
      path.join(root, "simulator", "verification", "corpus"),
      corpusPath,
    );
    applyCandidateFiles({
      destination: stage.work,
      work: candidate.work,
      changes: candidate.changes,
      expected: expectedHashes(candidate),
    });
    fs.rmSync(stageOutput, { recursive: true, force: true });
    fs.mkdirSync(stageOutput, { recursive: true });
    await buildVerifier(stage.work, targetDir);
    const verification = await verifyFocused({
      stageRoot: stage.work,
      task,
      stageOutput,
      targetDir,
    });
    const disposition = focusedDisposition(candidate.fingerprint, verification);
    if (!disposition.accepted) {
      await rejectCandidate(outputDir, candidate, disposition.note);
      return;
    }
    retainResolvedPrefix(stage.work, candidate.fingerprint);
    const gate = await runBroadGate({
      stageRoot: stage.work,
      stageOutput,
      targetDir,
      logPath,
    });
    const {
      parseFailureTraceNames,
    } = require(path.join(stage.work, "tools", "communication", "random_fidelity_batch_gate.js"));
    const candidateFailures = gate.code === 0
      ? []
      : parseFailureTraceNames(gate.fullOutput);
    const baseline = readJson(baselinePath);
    if (
      gate.code !== 0 &&
      (
        !baseline ||
        !gateHasNoNewFailures(baseline.failed_traces || [], candidateFailures)
      )
    ) {
      await rejectCandidate(
        outputDir,
        candidate,
        `broad corpus gate introduced a new failing trace; see ${logPath}`,
      );
      return;
    }
    applyCandidateFiles({
      destination: root,
      work: candidate.work,
      changes: candidate.changes,
      expected: expectedHashes(candidate),
    });
    const recheck = await queueCommand(
      outputDir,
      "recheck",
      candidate.fingerprint,
      candidate.worker,
    );
    if (recheck.code !== 0) {
      throw new Error(`post-promotion recheck failed: ${recheck.stderr || recheck.stdout}`);
    }
    updateCandidate(outputDir, candidate, {
      status: "promoted",
      promoted_at: new Date().toISOString(),
      focused_verification: {
        status: verification.status,
        fingerprint: verification.fingerprint,
      },
      gate_log: logPath,
    });
    writeJsonAtomic(baselinePath, {
      schema: 1,
      updated_at: new Date().toISOString(),
      failed_traces: candidateFailures,
      source: `promoted candidate ${candidate.fingerprint}`,
    });
  } catch (error) {
    await rejectCandidate(outputDir, candidate, error.message);
  }
}

async function main() {
  const outputDir = path.resolve(
    process.env.STS_RANDOM_OUTPUT_DIR || path.join(root, "random_traces_loop"),
  );
  const pollMs = Number.parseInt(process.env.STS_RANDOM_INTEGRATOR_POLL_MS || "2000", 10);
  if (!Number.isInteger(pollMs) || pollMs < 1) {
    throw new Error("STS_RANDOM_INTEGRATOR_POLL_MS must be a positive integer");
  }
  const recovered = recoverGatingCandidates(outputDir);
  console.log(JSON.stringify({
    status: "running",
    output_dir: outputDir,
    recovered_gating_candidates: recovered,
  }));
  for (;;) {
    if (fs.existsSync(path.join(outputDir, "repair_integration.pause"))) {
      await new Promise((resolve) => setTimeout(resolve, pollMs));
      continue;
    }
    const candidate = nextCandidate(readCandidates(outputDir));
    if (candidate) await integrateCandidate(outputDir, candidate);
    else await new Promise((resolve) => setTimeout(resolve, pollMs));
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exit(1);
  });
}

module.exports = {
  buildVerifier,
  focusedDisposition,
  gateHasNoNewFailures,
  nextCandidate,
  readCandidates,
  recoverGatingCandidates,
  snapshotPermanentCorpus,
  uvCommand,
  verifierBinaryPath,
};
