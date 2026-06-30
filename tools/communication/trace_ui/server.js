const fs = require("fs");
const http = require("http");
const crypto = require("crypto");
const net = require("net");
const path = require("path");
const url = require("url");

const port = Number.parseInt(process.env.PORT || "8787", 10);
const root = __dirname;
const sessionDir = process.env.TRACE_SESSION_DIR
  ? path.resolve(process.env.TRACE_SESSION_DIR)
  : path.resolve(root, "..", "session");
const commandPath = path.join(sessionDir, "next_command.txt");
const commandMetaPath = path.join(sessionDir, "next_command.json");

const mimeTypes = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
};

function readJson(name) {
  const filePath = path.join(sessionDir, name);
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    return { error: error.message, missing: error.code === "ENOENT" };
  }
}

function sendJson(res, status, value) {
  const body = JSON.stringify(value, null, 2);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
    "cache-control": "no-store",
  });
  res.end(body);
}

function parseBody(req) {
  return new Promise((resolve, reject) => {
    let body = "";
    req.on("data", (chunk) => {
      body += chunk;
      if (body.length > 64 * 1024) {
        reject(new Error("request body too large"));
        req.destroy();
      }
    });
    req.on("end", () => resolve(body));
    req.on("error", reject);
  });
}

function controlFromStatus(status) {
  const control = status?.control;
  if (!control || control.protocol !== "tcp-jsonl") return null;
  const port = Number.parseInt(control.port, 10);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) return null;
  return {
    host: control.host || "127.0.0.1",
    port,
    protocol: "tcp-jsonl",
  };
}

function controlRequest(control, payload, timeoutMs = 10000) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: control.host, port: control.port });
    let buffer = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("timed out waiting for bridge control response"));
    }, timeoutMs);
    socket.setEncoding("utf8");
    socket.on("connect", () => {
      socket.write(`${JSON.stringify(payload)}\n`);
    });
    socket.on("data", (chunk) => {
      buffer += chunk;
      const lineEnd = buffer.indexOf("\n");
      if (lineEnd < 0) return;
      const line = buffer.slice(0, lineEnd);
      clearTimeout(timer);
      socket.end();
      try {
        resolve(JSON.parse(line));
      } catch (error) {
        reject(error);
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

async function sendCommandViaControl(command, status, summary) {
  const control = controlFromStatus(status);
  if (!control) return null;
  const acquired = await controlRequest(control, {
    type: "acquire",
    owner_id: `trace-ui-${process.pid}`,
  });
  if (!acquired.ok || !acquired.owner_token) {
    throw new Error(acquired.error || "bridge control ownership rejected");
  }
  const payload = {
    type: "command",
    command,
    command_id: crypto.randomUUID(),
    expected_state_id: summary?.state_id || null,
    owner_token: acquired.owner_token,
    metadata: { source: "trace_ui" },
    wait_for_state_update: true,
    update_timeout_ms: 10000,
  };
  if (summary?.state_seq !== undefined) {
    payload.expected_state_seq = summary.state_seq;
  }
  let response;
  let releaseError = null;
  try {
    response = await controlRequest(control, payload, 12000);
    if (!response.ok) {
      throw new Error(response.error || "bridge control command rejected");
    }
  } finally {
    try {
      await controlRequest(control, {
        type: "release",
        owner_token: acquired.owner_token,
      });
    } catch (error) {
      releaseError = error.message || String(error);
    }
  }
  return {
    ok: true,
    transport: "tcp-jsonl",
    command_id: response.command_id,
    command: response.command,
    accepted_state_id: response.accepted_state_id,
    accepted_state_seq: response.accepted_state_seq,
    observed_update: response.observed_update
      ? {
        ok: response.observed_update.ok,
        state_id: response.observed_update.state_id,
        state_seq: response.observed_update.state_seq,
        step: response.observed_update.step,
        observed_changed: response.observed_update.observed_changed,
        application_status: response.observed_update.application_status,
        error: response.observed_update.error,
      }
      : null,
    release_error: releaseError,
  };
}

function sendCommandViaFiles(command, summary) {
  fs.mkdirSync(sessionDir, { recursive: true });
  const commandId = crypto.randomUUID();
  const commandMeta = {
    command_id: commandId,
    command,
    source_state_id: summary?.state_id || null,
    source_state_seq: summary?.state_seq ?? null,
    submitted_at: Date.now() / 1000,
    metadata: { source: "trace_ui" },
  };
  fs.writeFileSync(commandMetaPath, `${JSON.stringify(commandMeta)}\n`);
  fs.writeFileSync(commandPath, `${command}\n`);
  return { ok: true, transport: "file", command_id: commandId, command };
}

async function sendCommandToBridge(command) {
  const status = readJson("status.json");
  const summary = readJson("summary.json");
  const tcpResult = await sendCommandViaControl(command, status, summary);
  if (tcpResult) return tcpResult;
  return sendCommandViaFiles(command, summary);
}

async function handleApi(req, res, pathname) {
  if (req.method === "GET" && pathname === "/api/session") {
    return sendJson(res, 200, {
      status: readJson("status.json"),
      summary: readJson("summary.json"),
      state: readJson("current_state.json"),
    });
  }

  if (req.method === "POST" && pathname === "/api/command") {
    const body = await parseBody(req);
    let payload;
    try {
      payload = JSON.parse(body || "{}");
    } catch {
      return sendJson(res, 400, { error: "invalid JSON" });
    }

    const command = String(payload.command || "").trim();
    if (!command) {
      return sendJson(res, 400, { error: "command is required" });
    }
    if (command.length > 200) {
      return sendJson(res, 400, { error: "command is too long" });
    }

    return sendJson(res, 200, await sendCommandToBridge(command));
  }

  return sendJson(res, 404, { error: "not found" });
}

function serveStatic(res, pathname) {
  const safePath = pathname === "/" ? "/index.html" : pathname;
  const filePath = path.resolve(root, `.${safePath}`);
  if (!filePath.startsWith(root)) {
    res.writeHead(403);
    res.end("forbidden");
    return;
  }

  fs.readFile(filePath, (error, data) => {
    if (error) {
      res.writeHead(404);
      res.end("not found");
      return;
    }
    const contentType = mimeTypes[path.extname(filePath)] || "application/octet-stream";
    res.writeHead(200, { "content-type": contentType, "cache-control": "no-store" });
    res.end(data);
  });
}

const server = http.createServer(async (req, res) => {
  const { pathname } = url.parse(req.url);
  try {
    if (pathname.startsWith("/api/")) {
      await handleApi(req, res, pathname);
    } else {
      serveStatic(res, pathname);
    }
  } catch (error) {
    sendJson(res, 500, { error: error.message });
  }
});

if (require.main === module) {
  server.listen(port, () => {
    console.log(`Trace UI: http://localhost:${port}`);
    console.log(`Session: ${sessionDir}`);
  });
}

module.exports = {
  controlFromStatus,
  sendCommandToBridge,
  server,
};
