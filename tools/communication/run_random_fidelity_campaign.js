#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

const { resolveRandomFidelityOutputDir } = require("./random_fidelity_paths");

const collector = path.join(__dirname, "random_fidelity_collector.js");
const root = path.resolve(__dirname, "..", "..");
const maxRuns = Number.parseInt(process.env.STS_RANDOM_MAX_RUNS || "100", 10);
const seedPrefix = process.env.STS_RANDOM_GAME_SEED_PREFIX || "FIDL";
const retryDelayMs = Number.parseInt(process.env.STS_RANDOM_RETRY_DELAY_MS || "5000", 10);
const maxRetryDelayMs = Number.parseInt(
  process.env.STS_RANDOM_MAX_RETRY_DELAY_MS || "30000",
  10,
);
const failuresPerSeed = Number.parseInt(
  process.env.STS_RANDOM_FAILURES_PER_SEED || "3",
  10,
);
const outputDir = resolveRandomFidelityOutputDir();
const statusPath = path.join(outputDir, "campaign_status.json");
const indefinite = maxRuns <= 0;

if (!Number.isInteger(failuresPerSeed) || failuresPerSeed < 1) {
  throw new Error("STS_RANDOM_FAILURES_PER_SEED must be a positive integer");
}

function writeStatus(status) {
  fs.writeFileSync(
    statusPath,
    `${JSON.stringify({ campaign_pid: process.pid, updated_at: new Date().toISOString(), ...status }, null, 2)}\n`,
  );
}

function sleep(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function retryDelayForFailures(failures) {
  return Math.min(
    maxRetryDelayMs,
    retryDelayMs * (2 ** Math.min(failures - 1, 10)),
  );
}

function appendJsonl(filePath, value) {
  fs.appendFileSync(filePath, `${JSON.stringify(value)}\n`);
}

function firstUncollectedPolicySeed(directory, prefix, requested = 1) {
  let next = requested;
  const escapedPrefix = prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`^${escapedPrefix}\\d+-p(\\d+)-`);
  try {
    for (const name of fs.readdirSync(path.join(directory, "traces"))) {
      const match = pattern.exec(name);
      if (match) next = Math.max(next, Number.parseInt(match[1], 10) + 1);
    }
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  try {
    for (const line of fs
      .readFileSync(path.join(directory, "skipped_policy_seeds.jsonl"), "utf8")
      .split(/\r?\n/)
      .filter(Boolean)) {
      try {
        const entry = JSON.parse(line);
        if (entry.seed_prefix !== prefix) continue;
        const skipped = Number.parseInt(entry.policy_seed, 10);
        if (Number.isInteger(skipped)) next = Math.max(next, skipped + 1);
      } catch {}
    }
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  return next;
}

function isInfrastructureFailure(child) {
  const detail = String(child.stderr || child.error || "");
  if (/a command is already (?:queued|in flight)/i.test(detail)) return true;
  if (/bridge ownership rejected|controller owner_token/i.test(detail)) return true;
  return Number(child.elapsed_ms || 0) < 10_000 &&
    /ECONNREFUSED|timed out waiting for bridge control response|bridge is already in a run/i
      .test(detail);
}

/**
 * Finite and indefinite campaigns share skip-after-N. STS_RANDOM_MAX_RUNS is
 * the number of policy seeds consumed (successful collects or skipped seeds),
 * not collector retries of the same seed.
 */
function nextCampaignAction({
  indefinite,
  child,
  consecutiveFailures,
  failuresPerSeed: failureLimit,
}) {
  if (child.code === 0) return { type: "advance" };
  if (indefinite && isInfrastructureFailure(child)) return { type: "retry_infrastructure" };
  const failures = consecutiveFailures + 1;
  if (failures >= failureLimit) return { type: "skip_seed", consecutiveFailures: failures };
  return { type: "retry_seed", consecutiveFailures: failures };
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
        STS_RANDOM_ABANDON_EXISTING: "1",
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
  fs.mkdirSync(outputDir, { recursive: true });
  const requestedPolicySeed = Number.parseInt(process.env.STS_RANDOM_POLICY_SEED || "1", 10);
  const firstPolicySeed = firstUncollectedPolicySeed(
    outputDir,
    seedPrefix,
    requestedPolicySeed,
  );
  let offset = 0;
  let consecutiveFailures = 0;
  while (indefinite || offset < maxRuns) {
    const policySeed = firstPolicySeed + offset;
    const gameSeed = `${seedPrefix}${String(policySeed).padStart(5, "0")}`;
    const total = indefinite ? "infinite" : maxRuns;
    console.log(`\n=== random fidelity run ${offset + 1}/${total}: ${gameSeed}, policy ${policySeed} ===`);
    writeStatus({
      status: "running",
      mode: indefinite ? "indefinite" : "finite",
      run_number: offset + 1,
      game_seed: gameSeed,
      policy_seed: policySeed,
    });
    let child;
    try {
      child = await runCollector(gameSeed, policySeed);
    } catch (error) {
      child = { code: null, signal: null, error: error.message };
    }
    if (child.code !== 0) {
      const action = nextCampaignAction({
        indefinite,
        child,
        consecutiveFailures,
        failuresPerSeed,
      });
      if (action.type === "retry_infrastructure") {
        appendJsonl(path.join(outputDir, "campaign_failures.jsonl"), {
          recorded_at: new Date().toISOString(),
          game_seed: gameSeed,
          policy_seed: policySeed,
          failure_kind: "infrastructure",
          collector_exit_code: child.code,
          collector_signal: child.signal,
          elapsed_ms: child.elapsed_ms ?? null,
          detail: String(child.stderr || child.error || "").trim().slice(-4000),
        });
        writeStatus({
          status: "waiting_for_infrastructure",
          mode: "indefinite",
          run_number: offset + 1,
          game_seed: gameSeed,
          policy_seed: policySeed,
          collector_exit_code: child.code,
          collector_signal: child.signal,
          retry_delay_ms: retryDelayMs,
        });
        sleep(retryDelayMs);
        continue;
      }
      consecutiveFailures = action.consecutiveFailures;
      const effectiveRetryDelayMs = retryDelayForFailures(consecutiveFailures);
      const failure = {
        recorded_at: new Date().toISOString(),
        game_seed: gameSeed,
        policy_seed: policySeed,
        attempt: consecutiveFailures,
        collector_exit_code: child.code,
        collector_signal: child.signal,
        collector_error: child.error || null,
      };
      appendJsonl(path.join(outputDir, "campaign_failures.jsonl"), failure);
      if (action.type === "skip_seed") {
        appendJsonl(path.join(outputDir, "skipped_policy_seeds.jsonl"), {
          ...failure,
          seed_prefix: seedPrefix,
          reason: "collector failure retry limit reached",
        });
        writeStatus({
          status: "skipping_failed_seed",
          mode: indefinite ? "indefinite" : "finite",
          run_number: offset + 1,
          game_seed: gameSeed,
          policy_seed: policySeed,
          consecutive_failures: consecutiveFailures,
          failure_limit: failuresPerSeed,
        });
        consecutiveFailures = 0;
        offset += 1;
        continue;
      }
      writeStatus({
        status: "retrying_after_error",
        mode: indefinite ? "indefinite" : "finite",
        run_number: offset + 1,
        game_seed: gameSeed,
        policy_seed: policySeed,
        collector_exit_code: child.code,
        collector_signal: child.signal,
        collector_error: child.error || null,
        consecutive_failures: consecutiveFailures,
        retry_delay_ms: effectiveRetryDelayMs,
      });
      sleep(effectiveRetryDelayMs);
      continue;
    }
    consecutiveFailures = 0;
    offset += 1;
  }
  writeStatus({ status: "complete", mode: "finite", runs: maxRuns });
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
  isInfrastructureFailure,
  nextCampaignAction,
  retryDelayForFailures,
  runCollector,
};
