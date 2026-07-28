#!/usr/bin/env node

const fs = require("fs");
const net = require("net");
const path = require("path");
const { spawn, spawnSync } = require("child_process");

const defaultGameDir = "/mnt/d/SteamLibrary/steamapps/common/SlayTheSpire";
const defaultWorkshopJar =
  "D:\\SteamLibrary\\steamapps\\workshop\\content\\646570\\1605060445\\ModTheSpire.jar";
const defaultTasklist = "/mnt/c/Windows/System32/tasklist.exe";
const defaultTaskkill = "/mnt/c/Windows/System32/taskkill.exe";

function parseTasklistJavaPids(output) {
  return String(output)
    .split(/\r?\n/)
    .flatMap((line) => {
      const match = line.match(/^"java\.exe","(\d+)"/i);
      return match ? [Number.parseInt(match[1], 10)] : [];
    });
}

function windowsJavaPids(tasklistPath = defaultTasklist) {
  const result = spawnSync(
    tasklistPath,
    ["/FI", "IMAGENAME eq java.exe", "/FO", "CSV", "/NH"],
    { encoding: "utf8", windowsHide: true },
  );
  return new Set(parseTasklistJavaPids(result.stdout));
}

function terminateWindowsProcesses(processIds, taskkillPath = defaultTaskkill) {
  for (const processId of processIds) {
    spawnSync(
      taskkillPath,
      ["/PID", String(processId), "/T", "/F"],
      { encoding: "utf8", windowsHide: true },
    );
  }
}

function readControlEndpoint(sessionDir) {
  try {
    const status = JSON.parse(
      fs.readFileSync(path.join(sessionDir, "status.json"), "utf8"),
    );
    const port = Number.parseInt(status.control?.port, 10);
    if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
    return {
      host: status.control?.host || "127.0.0.1",
      port,
      clientPid: status.client_pid ?? null,
      pendingSinceMs: status.pending_command
        ? Date.parse(status.command_in_flight?.accepted_at || status.sent_at || "") ||
          Number(status.queued_command_meta?.submitted_at) * 1000 ||
          null
        : null,
    };
  } catch {
    return null;
  }
}

function probeControl(endpoint, timeoutMs = 2000) {
  if (!endpoint) return Promise.resolve(false);
  return new Promise((resolve) => {
    const socket = net.createConnection({
      host: endpoint.host,
      port: endpoint.port,
    });
    let settled = false;
    const finish = (healthy) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolve(healthy);
    };
    socket.once("connect", () => finish(true));
    socket.once("error", () => finish(false));
    socket.setTimeout(timeoutMs, () => finish(false));
  });
}

async function main() {
  const gameDir = path.resolve(process.env.STS_GAME_DIR || defaultGameDir);
  const sessionDir = path.resolve(
    process.env.STS_BRIDGE_SESSION_DIR ||
      path.join(__dirname, "session"),
  );
  const javaPath = path.resolve(
    process.env.STS_GAME_JAVA || path.join(gameDir, "jre", "bin", "java.exe"),
  );
  const workshopJar = process.env.STS_MODTHESPIRE_JAR || defaultWorkshopJar;
  const tasklistPath = process.env.STS_WINDOWS_TASKLIST || defaultTasklist;
  const taskkillPath = process.env.STS_WINDOWS_TASKKILL || defaultTaskkill;
  const pollMs = Number.parseInt(process.env.STS_GAME_WATCHDOG_POLL_MS || "5000", 10);
  const startupMs = Number.parseInt(
    process.env.STS_GAME_WATCHDOG_STARTUP_MS || "120000",
    10,
  );
  const failureLimit = Number.parseInt(
    process.env.STS_GAME_WATCHDOG_FAILURES || "3",
    10,
  );
  const pendingTimeoutMs = Number.parseInt(
    process.env.STS_GAME_WATCHDOG_PENDING_TIMEOUT_MS || "30000",
    10,
  );
  if (
    !Number.isInteger(pollMs) ||
    pollMs < 1 ||
    !Number.isInteger(startupMs) ||
    startupMs < 1 ||
    !Number.isInteger(failureLimit) ||
    failureLimit < 1 ||
    !Number.isInteger(pendingTimeoutMs) ||
    pendingTimeoutMs < 1
  ) {
    throw new Error("watchdog timing values must be positive integers");
  }

  const preexistingJavaPids = windowsJavaPids(tasklistPath);
  const launchedJavaPids = new Set();
  const child = spawn(
    javaPath,
    [
      "-jar",
      workshopJar,
      "--skip-launcher",
      "--skip-intro",
      "--mods",
      "basemod,CommunicationMod,abandon-run-control,superfastmode,verification-bootstrap",
    ],
    { cwd: gameDir, stdio: "inherit", windowsHide: true },
  );
  console.log(JSON.stringify({
    status: "game_started",
    pid: child.pid,
    session_dir: sessionDir,
  }));

  let stopping = false;
  let childExited = false;
  let childExitCode = null;
  const discoverLaunchedJavaPids = () => {
    for (const processId of windowsJavaPids(tasklistPath)) {
      if (!preexistingJavaPids.has(processId)) launchedJavaPids.add(processId);
    }
  };
  const terminateGame = () => {
    discoverLaunchedJavaPids();
    terminateWindowsProcesses(launchedJavaPids, taskkillPath);
    if (!childExited) child.kill("SIGKILL");
  };
  child.once("exit", (code, signal) => {
    childExited = true;
    childExitCode = code;
    console.error(JSON.stringify({ status: "game_exited", code, signal }));
  });
  child.once("error", (error) => {
    childExited = true;
    childExitCode = 1;
    console.error(error.stack || error);
  });
  const stop = (signal) => {
    stopping = true;
    discoverLaunchedJavaPids();
    terminateWindowsProcesses(launchedJavaPids, taskkillPath);
    if (!childExited) child.kill(signal);
  };
  process.once("SIGTERM", () => stop("SIGTERM"));
  process.once("SIGINT", () => stop("SIGINT"));

  const startupDeadline = Date.now() + startupMs;
  let becameHealthy = false;
  let failures = 0;
  for (;;) {
    discoverLaunchedJavaPids();
    if (childExited) {
      terminateWindowsProcesses(launchedJavaPids, taskkillPath);
      process.exit(stopping ? 0 : childExitCode || 1);
    }
    const endpoint = readControlEndpoint(sessionDir);
    const stalePending = endpoint?.pendingSinceMs !== null &&
      Date.now() - endpoint.pendingSinceMs >= pendingTimeoutMs;
    const healthy = !stalePending && await probeControl(endpoint);
    if (healthy) {
      if (!becameHealthy) {
        console.log(JSON.stringify({
          status: "bridge_healthy",
          endpoint,
        }));
      }
      becameHealthy = true;
      failures = 0;
    } else if (becameHealthy || Date.now() >= startupDeadline) {
      failures += 1;
      console.error(JSON.stringify({
        status: "bridge_probe_failed",
        failures,
        failure_limit: failureLimit,
        endpoint,
        stale_pending_command: Boolean(stalePending),
      }));
    }
    if (failures >= failureLimit) {
      console.error("CommunicationMod bridge stayed unavailable; restarting game");
      terminateGame();
      setTimeout(() => process.exit(1), 2000).unref();
      await new Promise((resolve) => child.once("exit", resolve));
      process.exit(1);
    }
    await new Promise((resolve) => setTimeout(resolve, pollMs));
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exit(1);
  });
}

module.exports = {
  parseTasklistJavaPids,
  probeControl,
  readControlEndpoint,
  terminateWindowsProcesses,
  windowsJavaPids,
};
