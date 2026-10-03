#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");
const crypto = require("crypto");
const { parseIntegerEnv } = require("./random_fidelity_collector");

const collector = path.join(__dirname, "random_fidelity_collector.js");
const root = path.resolve(__dirname, "..", "..", "..");
const maxRuns = parseIntegerEnv("STS_RANDOM_MAX_RUNS", 100, 0);
const seedPrefix = process.env.STS_RANDOM_GAME_SEED_PREFIX || "FIDL";
const outputDir = path.resolve(
  process.env.STS_RANDOM_OUTPUT_DIR || path.join(root, "target", "random-fidelity"),
);
const statusPath = path.join(outputDir, "campaign_status.json");
const indefinite = maxRuns <= 0;

fs.mkdirSync(outputDir, { recursive: true });

function writeStatus(status) {
  fs.writeFileSync(
    statusPath,
    `${JSON.stringify({ campaign_pid: process.pid, updated_at: new Date().toISOString(), ...status }, null, 2)}\n`,
  );
}

function appendJsonl(filePath, value) {
  fs.appendFileSync(filePath, `${JSON.stringify(value)}\n`);
}

function traceCollectionMetadata(filePath) {
  const descriptor = fs.openSync(filePath, "r");
  try {
    const buffer = Buffer.alloc(65_536);
    const bytesRead = fs.readSync(descriptor, buffer, 0, buffer.length, 0);
    const firstLine = buffer.subarray(0, bytesRead).toString("utf8").split(/\r?\n/, 1)[0];
    if (!firstLine) return null;
    const record = JSON.parse(firstLine);
    return record?.type === "metadata" && record?.collection ? record.collection : null;
  } catch {
    return null;
  } finally {
    fs.closeSync(descriptor);
  }
}

// Resume only a clean, sealed campaign. Missing/broken evidence is not a gap
// that authorizes retrying an interrupted run. Never remove an old owner lock.
function firstUncollectedPolicySeed(directory, prefix, requested = 1) {
  const collected = new Set();
  const sealed = new Map();
  const freshDirectoryRequired = detail => new Error(`${detail}; explicit review and a fresh output directory required`);
  for (const name of ["campaign_failures.jsonl"]) {
    try {
      if (fs.readFileSync(path.join(directory, name), "utf8").trim()) {
        throw freshDirectoryRequired("campaign has a recorded failure");
      }
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  try {
    const status = JSON.parse(fs.readFileSync(path.join(directory, "campaign_status.json"), "utf8"));
    if (status.status === "failed" || (status.status === "running" && status.campaign_pid !== process.pid)) {
      throw freshDirectoryRequired("campaign is failed or owned by another/interrupted supervisor");
    }
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  try {
    for (const line of fs.readFileSync(path.join(directory, "ledger.jsonl"), "utf8").split(/\r?\n/).filter(Boolean)) {
      const entry = JSON.parse(line);
      if (entry.kind === "collected_incomplete") throw freshDirectoryRequired("campaign has an incomplete capture");
      if (entry.kind !== "collected") continue;
      if (entry.terminal_reason !== "game_over" || typeof entry.trace !== "string"
          || !/^[a-f0-9]{64}$/.test(entry.trace_sha256)) {
        throw freshDirectoryRequired("campaign has an unsealed collection record");
      }
      const trace = path.resolve(entry.trace);
      if (path.dirname(trace) !== path.resolve(directory, "traces")) {
        throw freshDirectoryRequired(`sealed trace is outside this campaign: ${trace}`);
      }
      if (!fs.existsSync(trace)) throw freshDirectoryRequired(`sealed trace is missing: ${trace}`);
      sealed.set(trace, entry);
    }
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const escapedPrefix = prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`^${escapedPrefix}\\d+-p(\\d+)-.*\\.jsonl$`);
  let names = [];
  try {
    names = fs.readdirSync(path.join(directory, "traces"));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    if (sealed.size) throw freshDirectoryRequired("sealed campaign trace directory is missing");
  }
  // Only a never-created traces directory is an allowed ENOENT. Losing an
  // individual sealed file during inspection must fail, not authorize retry.
  for (const name of names) {
    const match = pattern.exec(name);
    if (!match) continue;
    const policySeed = Number(match[1]);
    if (!Number.isSafeInteger(policySeed) || policySeed < 1) throw freshDirectoryRequired(`invalid policy seed: ${name}`);
    const trace = path.resolve(directory, "traces", name);
    const entry = sealed.get(trace);
    if (!entry || entry.policy_seed !== policySeed) throw freshDirectoryRequired(`unsealed trace: ${trace}`);
    const metadata = traceCollectionMetadata(trace);
    if (metadata?.policy_seed === policySeed
        && metadata?.game_seed === `${prefix}${String(policySeed).padStart(5, "0")}`
        && entry.game_seed === metadata.game_seed) {
      const digest = crypto.createHash("sha256").update(fs.readFileSync(trace)).digest("hex");
      if (digest !== entry.trace_sha256) throw new Error(`sealed trace hash mismatch: ${trace}`);
      collected.add(policySeed);
    } else {
      throw freshDirectoryRequired(`sealed trace metadata mismatch: ${trace}`);
    }
  }
  let next = requested;
  while (collected.has(next)) next += 1;
  return next;
}

function runCollector(gameSeed, policySeed) {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now();
    const child = spawn(process.execPath, [collector], {
      cwd: root,
      env: {
        ...process.env,
        STS_GAME_SEED: gameSeed,
        STS_RANDOM_POLICY_SEED: String(policySeed),
      },
      stdio: ["inherit", "inherit", "pipe"],
      windowsHide: true,
    });
    let stderr = "";
    child.stderr.on("data", (chunk) => {
      process.stderr.write(chunk);
      stderr = `${stderr}${chunk}`.slice(-65_536);
    });
    child.once("error", reject);
    child.once("close", (code, signal) => {
      resolve({ code, signal, stderr, elapsed_ms: Date.now() - startedAt });
    });
  });
}

async function main() {
  const lockPath = path.join(outputDir, "campaign.lock");
  const lockIdentity = JSON.stringify({ pid: process.pid, id: crypto.randomUUID(), started_at: new Date().toISOString() });
  const lock = fs.openSync(lockPath, "wx");
  fs.writeFileSync(lock, lockIdentity + "\n");
  fs.closeSync(lock);
  // Failure/interruption preserves this lock. No PID-based stale takeover.
  let nextPolicySeed = parseIntegerEnv("STS_RANDOM_POLICY_SEED", 1);
  let completedRuns = 0;
  while (indefinite || completedRuns < maxRuns) {
    const policySeed = firstUncollectedPolicySeed(
      outputDir,
      seedPrefix,
      nextPolicySeed,
    );
    const gameSeed = `${seedPrefix}${String(policySeed).padStart(5, "0")}`;
    const total = indefinite ? "infinite" : maxRuns;
    console.log(
      `\n=== random fidelity run ${completedRuns + 1}/${total}: ${gameSeed}, policy ${policySeed} ===`,
    );
    writeStatus({
      status: "running",
      mode: indefinite ? "indefinite" : "finite",
      run_number: completedRuns + 1,
      game_seed: gameSeed,
      policy_seed: policySeed,
    });
    let child;
    try {
      child = await runCollector(gameSeed, policySeed);
    } catch (error) {
      child = { code: null, signal: null, error: error.message };
    }
    if (child.code === 0) {
      try {
        nextPolicySeed = firstUncollectedPolicySeed(outputDir, seedPrefix, policySeed);
        if (nextPolicySeed === policySeed) throw new Error("collector exited successfully without a sealed natural-completion trace");
      } catch (error) {
        child = { ...child, validation_error: error.message };
      }
    }
    if (child.code !== 0 || child.validation_error) {
      const failure = {
        recorded_at: new Date().toISOString(),
        game_seed: gameSeed,
        policy_seed: policySeed,
        attempt: 1,
        validation_error: child.validation_error || null,
        collector_exit_code: child.code,
        collector_signal: child.signal,
        collector_error: child.error || null,
      };
      appendJsonl(path.join(outputDir, "campaign_failures.jsonl"), failure);
      writeStatus({
        status: "failed",
        mode: indefinite ? "indefinite" : "finite",
        run_number: completedRuns + 1,
        game_seed: gameSeed,
        policy_seed: policySeed,
        collector_exit_code: child.code,
        collector_signal: child.signal,
        collector_error: child.error || null,
        consecutive_failures: 1,
        validation_error: child.validation_error || null,
        retry_delay_ms: null,
      });
      // Includes indefinite mode: no implicit retry, takeover, or orphan recovery.
      process.exitCode = child.code || 1;
      return;
    }
    completedRuns += 1;
  }
  if (fs.readFileSync(lockPath, "utf8").trim() !== lockIdentity) throw new Error("campaign owner lock changed");
  writeStatus({ status: "complete", mode: "finite", captured_runs: completedRuns });
  fs.unlinkSync(lockPath);
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exit(1);
  });
}

module.exports = {
  appendJsonl,
  firstUncollectedPolicySeed,
  runCollector,
};
