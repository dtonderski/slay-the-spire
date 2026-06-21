const fs = require("fs");
const http = require("http");
const path = require("path");
const url = require("url");

const port = Number.parseInt(process.env.PORT || "8787", 10);
const root = __dirname;
const sessionDir = path.resolve(root, "..", "session");
const commandPath = path.join(sessionDir, "next_command.txt");

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

    fs.mkdirSync(sessionDir, { recursive: true });
    fs.writeFileSync(commandPath, `${command}\n`);
    return sendJson(res, 200, { ok: true, command });
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

server.listen(port, () => {
  console.log(`Trace UI: http://localhost:${port}`);
  console.log(`Session: ${sessionDir}`);
});
