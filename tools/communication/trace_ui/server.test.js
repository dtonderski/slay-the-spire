const assert = require("assert");
const fs = require("fs");
const net = require("net");
const os = require("os");
const path = require("path");

const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-trace-ui-"));
const sessionDir = path.join(root, "session");
fs.mkdirSync(sessionDir, { recursive: true });
process.env.TRACE_SESSION_DIR = sessionDir;

const { sendCommandToBridge } = require("./server");

function writeJson(name, value) {
  fs.writeFileSync(path.join(sessionDir, name), `${JSON.stringify(value)}\n`);
}

async function withControlServer(handler, callback) {
  const received = [];
  const server = net.createServer((socket) => {
    socket.setEncoding("utf8");
    let buffer = "";
    socket.on("data", (chunk) => {
      buffer += chunk;
      const lineEnd = buffer.indexOf("\n");
      if (lineEnd < 0) return;
      const payload = JSON.parse(buffer.slice(0, lineEnd));
      received.push(payload);
      const response = handler(payload);
      socket.write(`${JSON.stringify(response)}\n`);
      socket.end();
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    return await callback({
      port: server.address().port,
      received,
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testTcpCommandPathUsesExpectedStateAndNoFileWrite() {
  await withControlServer((payload) => {
    if (payload.type === "acquire") {
      return { ok: true, owner_id: payload.owner_id, owner_token: "owner-token" };
    }
    return {
      ok: true,
      command_id: payload.command_id,
      command: payload.command,
      accepted_state_id: payload.expected_state_id,
      accepted_state_seq: payload.expected_state_seq,
      observed_update: {
        ok: true,
        state_id: "state-2",
        state_seq: 8,
        step: 3,
      },
    };
  }, async (control) => {
    writeJson("status.json", {
      status: "waiting",
      control: { protocol: "tcp-jsonl", host: "127.0.0.1", port: control.port },
    });
    writeJson("summary.json", {
      state_id: "state-1",
      state_seq: 7,
      ready_for_command: true,
      available_commands: ["choose", "state"],
    });

    const result = await sendCommandToBridge("CHOOSE 0");

    assert.strictEqual(result.transport, "tcp-jsonl");
    assert.strictEqual(result.accepted_state_id, "state-1");
    assert.strictEqual(result.accepted_state_seq, 7);
    assert.strictEqual(result.observed_update.state_id, "state-2");
    assert.strictEqual(fs.existsSync(path.join(sessionDir, "next_command.txt")), false);
    assert.strictEqual(control.received.length, 2);
    assert.strictEqual(control.received[0].type, "acquire");
    assert.strictEqual(control.received[0].owner_id, `trace-ui-${process.pid}`);
    assert.strictEqual(control.received[1].type, "command");
    assert.strictEqual(control.received[1].expected_state_id, "state-1");
    assert.strictEqual(control.received[1].expected_state_seq, 7);
    assert.strictEqual(control.received[1].metadata.source, "trace_ui");
    assert.strictEqual(control.received[1].wait_for_state_update, true);
  });
}

async function testFileFallbackWritesCommandMetadata() {
  fs.rmSync(sessionDir, { recursive: true, force: true });
  fs.mkdirSync(sessionDir, { recursive: true });
  writeJson("status.json", { status: "waiting" });
  writeJson("summary.json", {
    state_id: "state-file",
    ready_for_command: true,
    available_commands: ["state"],
  });

  const result = await sendCommandToBridge("state");
  const commandMeta = JSON.parse(fs.readFileSync(path.join(sessionDir, "next_command.json"), "utf8"));

  assert.strictEqual(result.transport, "file");
  assert.strictEqual(fs.readFileSync(path.join(sessionDir, "next_command.txt"), "utf8"), "state\n");
  assert.strictEqual(commandMeta.command, "state");
  assert.strictEqual(commandMeta.source_state_id, "state-file");
  assert.strictEqual(commandMeta.metadata.source, "trace_ui");
}

Promise.resolve()
  .then(testTcpCommandPathUsesExpectedStateAndNoFileWrite)
  .then(testFileFallbackWritesCommandMetadata)
  .then(() => {
    console.log("trace_ui server tests passed");
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  })
  .finally(() => {
    fs.rmSync(root, { recursive: true, force: true });
  });
