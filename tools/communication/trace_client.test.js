#!/usr/bin/env node

const assert = require("assert");
const { spawn } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..", "..");
const traceClientPath = path.join(__dirname, "trace_client.js");

function waitFor(predicate, timeoutMs = 3000) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    function poll() {
      try {
        const value = predicate();
        if (value) {
          resolve(value);
          return;
        }
      } catch (error) {
        reject(error);
        return;
      }
      if (Date.now() - started > timeoutMs) {
        reject(new Error("timed out waiting for condition"));
        return;
      }
      setTimeout(poll, 25);
    }
    poll();
  });
}

function readJsonLines(filePath) {
  return fs.readFileSync(filePath, "utf8")
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

async function testCommandMetadataIsPreservedInTraceActions() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-client-"));
  const sessionDir = path.join(root, "session");
  const outDir = path.join(root, "out");
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.mkdirSync(outDir, { recursive: true });

  const child = spawn(process.execPath, [traceClientPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      TRACE_SESSION_DIR: sessionDir,
      TRACE_OUT_DIR: outDir,
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  try {
    await waitFor(() => stdout.includes("ready\n"));
    child.stdin.write(`${JSON.stringify({
      in_game: true,
      ready_for_command: true,
      available_commands: ["choose"],
      game_state: {
        screen_type: "EVENT",
        floor: 2,
        choice_list: ["Pray"],
      },
    })}\n`);

    await waitFor(() => fs.existsSync(path.join(sessionDir, "status.json"))
      && JSON.parse(fs.readFileSync(path.join(sessionDir, "status.json"), "utf8")).status === "waiting");

    const metadata = {
      command_id: "cmd-guided-1",
      command: "CHOOSE 0",
      source_state_id: "bridge-state-1",
      metadata: {
        source: "guided_collector",
        collector_id: "collector-1",
      },
    };
    fs.writeFileSync(path.join(sessionDir, "next_command.json"), `${JSON.stringify(metadata)}\n`);
    fs.writeFileSync(path.join(sessionDir, "next_command.txt"), "CHOOSE 0\n");

    await waitFor(() => stdout.includes("CHOOSE 0\n"));
    child.stdin.end();
    await new Promise((resolve) => child.on("exit", resolve));

    const traceFiles = fs.readdirSync(outDir).filter((name) => name.endsWith(".jsonl"));
    assert.strictEqual(traceFiles.length, 1);
    const records = readJsonLines(path.join(outDir, traceFiles[0]));
    const action = records.find((record) => record.type === "action");
    assert.ok(action, `missing action record; stderr=${stderr}`);
    assert.strictEqual(action.command, "CHOOSE 0");
    assert.deepStrictEqual(action.command_meta, metadata);
    assert.strictEqual(fs.existsSync(path.join(sessionDir, "next_command.json")), false);
  } finally {
    if (!child.killed && child.exitCode === null) child.kill();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

testCommandMetadataIsPreservedInTraceActions()
  .then(() => {
    console.log("trace_client tests passed");
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
