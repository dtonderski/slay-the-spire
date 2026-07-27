#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const defaultTracePath = path.join(__dirname, "session", "raw_bridge_current.jsonl");

function acceptedCommands(content) {
  const commands = [];
  for (const line of content.split(/\r?\n/)) {
    if (!line.trim()) continue;
    try {
      const row = JSON.parse(line.replaceAll("\0", ""));
      if (row.type === "command_accept" && row.accepted_at && row.command) {
        commands.push(row);
      }
    } catch {
      // The bridge may be appending the final JSONL row while it is read.
    }
  }
  return commands;
}

function actionTimings(content) {
  const commands = acceptedCommands(content);
  const timings = [];
  let previousAction = null;
  let statePollTimes = [];
  const recentActionTimes = [];
  for (const row of commands) {
    const verb = String(row.command).trim().split(/\s+/)[0].toUpperCase();
    if (verb === "STATE") {
      const acceptedMs = Date.parse(row.accepted_at);
      if (previousAction && Number.isFinite(acceptedMs)) statePollTimes.push(acceptedMs);
      continue;
    }
    const acceptedMs = Date.parse(row.accepted_at);
    if (!Number.isFinite(acceptedMs)) continue;
    recentActionTimes.push(acceptedMs);
    if (recentActionTimes.length > 10) recentActionTimes.shift();
    const rollingSpanMs = recentActionTimes.length > 1
      ? recentActionTimes.at(-1) - recentActionTimes[0]
      : 0;
    timings.push({
      accepted_at: row.accepted_at,
      command: row.command,
      gap_ms: previousAction ? acceptedMs - previousAction.acceptedMs : null,
      state_polls: previousAction ? statePollTimes.length : 0,
      first_poll_ms: previousAction && statePollTimes.length > 0
        ? statePollTimes[0] - previousAction.acceptedMs
        : null,
      settle_poll_span_ms: statePollTimes.length > 1
        ? statePollTimes.at(-1) - statePollTimes[0]
        : 0,
      post_poll_ms: previousAction && statePollTimes.length > 0
        ? acceptedMs - statePollTimes.at(-1)
        : null,
      rolling_aps_10: rollingSpanMs > 0
        ? Number((((recentActionTimes.length - 1) * 1000) / rollingSpanMs).toFixed(3))
        : null,
    });
    previousAction = { acceptedMs };
    statePollTimes = [];
  }
  return timings;
}

function readTimings(tracePath) {
  try {
    return actionTimings(fs.readFileSync(tracePath, "utf8"));
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

function main() {
  const args = process.argv.slice(2);
  const watch = args.includes("--watch");
  const fromNow = args.includes("--from-now");
  const traceArg = args.find((arg) => !["--watch", "--from-now"].includes(arg));
  const tracePath = path.resolve(traceArg || defaultTracePath);
  let emitted = fromNow ? readTimings(tracePath).length : 0;

  const publish = () => {
    const timings = readTimings(tracePath);
    if (timings.length < emitted) emitted = 0;
    for (const timing of timings.slice(emitted)) {
      process.stdout.write(`${JSON.stringify(timing)}\n`);
    }
    emitted = timings.length;
  };

  publish();
  if (!watch) return;
  process.stderr.write(`Watching gameplay action timing: ${tracePath}\n`);
  const timer = setInterval(publish, 250);
  const stop = () => {
    clearInterval(timer);
    process.exit(0);
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);
}

if (require.main === module) main();

module.exports = { acceptedCommands, actionTimings };
