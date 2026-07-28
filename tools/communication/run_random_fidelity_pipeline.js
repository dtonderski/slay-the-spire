#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

const root = path.resolve(__dirname, "..", "..");

function verifierWorkerCount(value = process.env.STS_RANDOM_VERIFY_WORKERS || "1") {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error("STS_RANDOM_VERIFY_WORKERS must be a positive integer");
  }
  return parsed;
}

function repairWorkerCount(value = process.env.STS_RANDOM_REPAIR_WORKERS || "2") {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error("STS_RANDOM_REPAIR_WORKERS must be a positive integer");
  }
  return parsed;
}

function serviceSpecs(
  workerCount = verifierWorkerCount(),
  repairCount = repairWorkerCount(),
) {
  const specs = [
    {
      name: "campaign",
      script: "run_random_fidelity_campaign.js",
      env: {
        STS_RANDOM_MAX_RUNS: process.env.STS_RANDOM_MAX_RUNS || "0",
        STS_RANDOM_DEFER_VERIFICATION: "1",
      },
    },
    {
      name: "corpus-promoter",
      script: "random_fidelity_corpus_promoter.js",
      env: {},
    },
  ];
  for (let index = 0; index < workerCount; index += 1) {
    specs.push({
      name: `verifier-${index}`,
      script: "random_fidelity_verifier.js",
      env: {
        STS_RANDOM_VERIFY_WORKERS: String(workerCount),
        STS_RANDOM_VERIFY_WORKER_INDEX: String(index),
      },
    });
  }
  specs.push({
    name: "repair-integrator",
    script: "random_fidelity_repair_integrator.js",
    env: {},
  });
  for (let index = 0; index < repairCount; index += 1) {
    specs.push({
      name: `repair-${index}`,
      script: "random_fidelity_repair_worker.js",
      env: {
        STS_RANDOM_REPAIR_WORKERS: String(repairCount),
        STS_RANDOM_REPAIR_WORKER_INDEX: String(index),
      },
    });
  }
  return specs;
}

function restartDelayMs(consecutiveFailures) {
  return Math.min(30_000, 1_000 * (2 ** Math.min(consecutiveFailures, 5)));
}

function writeJsonAtomic(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const temporary = `${filePath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
  fs.renameSync(temporary, filePath);
}

function main() {
  const outputDir = path.resolve(
    process.env.STS_RANDOM_OUTPUT_DIR || path.join(root, "random_traces_loop"),
  );
  const logDir = path.join(outputDir, "pipeline_logs");
  const statusPath = path.join(outputDir, "pipeline_status.json");
  const workerCount = verifierWorkerCount();
  const repairCount = repairWorkerCount();
  const states = new Map();
  let stopping = false;

  fs.mkdirSync(logDir, { recursive: true });

  const publish = () => {
    writeJsonAtomic(statusPath, {
      schema: 1,
      supervisor_pid: process.pid,
      updated_at: new Date().toISOString(),
      stopping,
      verifier_workers: workerCount,
      repair_workers: repairCount,
      services: Object.fromEntries(
        [...states].map(([name, state]) => {
          const { child, ...serializable } = state;
          return [name, serializable];
        }),
      ),
    });
  };

  const start = (spec) => {
    if (stopping) return;
    const previous = states.get(spec.name) || {
      restarts: 0,
      consecutive_failures: 0,
    };
    const logPath = path.join(logDir, `${spec.name}.log`);
    const log = fs.createWriteStream(logPath, { flags: "a" });
    const child = spawn(process.execPath, [path.join(__dirname, spec.script)], {
      cwd: root,
      env: {
        ...process.env,
        STS_RANDOM_OUTPUT_DIR: outputDir,
        ...spec.env,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const startedAt = new Date().toISOString();
    child.stdout.pipe(log, { end: false });
    child.stderr.pipe(log, { end: false });
    states.set(spec.name, {
      ...previous,
      status: "running",
      pid: child.pid,
      started_at: startedAt,
      log: logPath,
      child,
    });
    publish();

    child.once("exit", (code, signal) => {
      log.end();
      const ranForMs = Date.now() - Date.parse(startedAt);
      const consecutiveFailures = ranForMs >= 60_000
        ? 1
        : Number(previous.consecutive_failures || 0) + 1;
      const delay = restartDelayMs(consecutiveFailures);
      states.set(spec.name, {
        ...states.get(spec.name),
        status: stopping ? "stopped" : "restarting",
        pid: null,
        child: null,
        exited_at: new Date().toISOString(),
        exit_code: code,
        signal,
        restarts: Number(previous.restarts || 0) + (stopping ? 0 : 1),
        consecutive_failures: consecutiveFailures,
        restart_delay_ms: stopping ? null : delay,
      });
      publish();
      if (!stopping) setTimeout(() => start(spec), delay);
    });
  };

  const stop = () => {
    if (stopping) return;
    stopping = true;
    for (const state of states.values()) state.child?.kill("SIGTERM");
    publish();
  };
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);

  for (const spec of serviceSpecs(workerCount, repairCount)) start(spec);
  console.log(JSON.stringify({
    status: "running",
    output_dir: outputDir,
    verifier_workers: workerCount,
    repair_workers: repairCount,
    status_path: statusPath,
  }));
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(error.stack || error);
    process.exit(1);
  }
}

module.exports = {
  repairWorkerCount,
  restartDelayMs,
  serviceSpecs,
  verifierWorkerCount,
};
