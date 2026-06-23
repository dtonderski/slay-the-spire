#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const sessionDir = path.join(__dirname, "session");
const commandPath = path.join(sessionDir, "next_command.txt");
const summaryPath = path.join(sessionDir, "summary.json");
const statusPath = path.join(sessionDir, "status.json");

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function statMtimeMs(filePath) {
  try {
    return fs.statSync(filePath).mtimeMs;
  } catch {
    return null;
  }
}

function bridgeProbeResultFrom({ commandExisted, consumed, summaryChanged, statusChanged, cleanedUp }) {
  const problems = [];
  if (commandExisted) problems.push("next_command.txt already exists");
  if (!commandExisted && !consumed) problems.push("bridge did not consume probe command");
  return {
    ok: problems.length === 0,
    problems,
    consumed,
    summaryChanged,
    statusChanged,
    cleanedUp,
  };
}

async function probeBridge(options = {}) {
  const waitMs = options.waitMs ?? Number.parseInt(process.env.STS_BRIDGE_PROBE_WAIT_MS || "3000", 10);
  const command = options.command || "state";
  if (fs.existsSync(commandPath)) {
    return bridgeProbeResultFrom({
      commandExisted: true,
      consumed: false,
      summaryChanged: false,
      statusChanged: false,
      cleanedUp: false,
    });
  }

  const beforeSummaryMtime = statMtimeMs(summaryPath);
  const beforeStatusMtime = statMtimeMs(statusPath);
  fs.writeFileSync(commandPath, `${command}\n`);
  await sleep(waitMs);

  const consumed = !fs.existsSync(commandPath);
  let cleanedUp = false;
  if (!consumed && command === "state") {
    fs.unlinkSync(commandPath);
    cleanedUp = true;
  }

  return bridgeProbeResultFrom({
    commandExisted: false,
    consumed,
    summaryChanged: statMtimeMs(summaryPath) !== beforeSummaryMtime,
    statusChanged: statMtimeMs(statusPath) !== beforeStatusMtime,
    cleanedUp,
  });
}

if (require.main === module) {
  probeBridge()
    .then((result) => {
      const summary = readJson(summaryPath);
      const status = readJson(statusPath);
      console.log(
        JSON.stringify(
          {
            ...result,
            summary: summary
              ? {
                  step: summary.step ?? null,
                  client_pid: summary.client_pid ?? null,
                  screen_type: summary.screen_type ?? null,
                  floor: summary.floor ?? null,
                  ready_for_command: summary.ready_for_command ?? null,
                }
              : null,
            status: status
              ? {
                  step: status.step ?? null,
                  client_pid: status.client_pid ?? null,
                  status: status.status ?? null,
                  command: status.command ?? null,
                }
              : null,
          },
          null,
          2,
        ),
      );
      process.exit(result.ok ? 0 : 1);
    })
    .catch((error) => {
      console.error(error.stack || String(error));
      process.exit(1);
    });
}

module.exports = {
  bridgeProbeResultFrom,
  probeBridge,
};
