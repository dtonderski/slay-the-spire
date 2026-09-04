#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const net = require("net");
const path = require("path");
const {
  parseTasklistJavaPids,
  probeControl,
  readControlEndpoint,
} = require("./random_fidelity_game_watchdog");

async function main() {
  assert.deepStrictEqual(
    parseTasklistJavaPids([
      '"Image Name","PID","Session Name","Session#","Mem Usage"',
      '"java.exe","1234","Console","1","500,000 K"',
      '"other.exe","55","Console","1","1 K"',
      '"java.exe","9876","Console","1","700,000 K"',
    ].join("\r\n")),
    [1234, 9876],
  );
  const root = fs.mkdtempSync(path.join("/tmp", "sts-game-watchdog-"));
  try {
    assert.strictEqual(readControlEndpoint(root), null);
    fs.writeFileSync(
      path.join(root, "status.json"),
      JSON.stringify({ control: { host: "127.0.0.1", port: 0 } }),
    );
    assert.strictEqual(readControlEndpoint(root), null);

    const server = net.createServer();
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const address = server.address();
    fs.writeFileSync(
      path.join(root, "status.json"),
      JSON.stringify({
        client_pid: 42,
        control: { host: "127.0.0.1", port: address.port },
      }),
    );
    assert.deepStrictEqual(readControlEndpoint(root), {
      host: "127.0.0.1",
      port: address.port,
      clientPid: 42,
      pendingSinceMs: null,
    });
    assert.strictEqual(await probeControl(readControlEndpoint(root), 1000), true);
    fs.writeFileSync(
      path.join(root, "status.json"),
      JSON.stringify({
        client_pid: 42,
        pending_command: true,
        queued_command_meta: { submitted_at: 123.5 },
        control: { host: "127.0.0.1", port: address.port },
      }),
    );
    assert.strictEqual(readControlEndpoint(root).pendingSinceMs, 123500);
    await new Promise((resolve) => server.close(resolve));
    assert.strictEqual(await probeControl(readControlEndpoint(root), 100), false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
  console.log("random fidelity game watchdog tests passed");
}

main().catch((error) => {
  console.error(error.stack || error);
  process.exit(1);
});
