#!/usr/bin/env node

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

const managedPrefixes = [
  "simulator/Cargo.lock",
  "simulator/Cargo.toml",
  "simulator/crates/",
  "simulator/docs/",
];

function normalizedRelative(root, filePath) {
  return path.relative(root, filePath).split(path.sep).join("/");
}

function isManagedPath(relativePath) {
  return managedPrefixes.some((prefix) =>
    prefix.endsWith("/") ? relativePath.startsWith(prefix) : relativePath === prefix);
}

function hashFile(filePath) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

function snapshotFiles(directory) {
  const result = {};
  const visit = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        visit(fullPath);
      } else if (entry.isFile()) {
        const relativePath = normalizedRelative(directory, fullPath);
        if (isManagedPath(relativePath)) result[relativePath] = hashFile(fullPath);
      }
    }
  };
  visit(directory);
  return result;
}

function candidateChanges(before, after) {
  return [...new Set([...Object.keys(before), ...Object.keys(after)])]
    .sort()
    .flatMap((relativePath) => {
      if (before[relativePath] === after[relativePath]) return [];
      return [{
        path: relativePath,
        operation: before[relativePath] === undefined
          ? "add"
          : after[relativePath] === undefined
            ? "delete"
            : "modify",
        baseline_sha256: before[relativePath] || null,
        candidate_sha256: after[relativePath] || null,
      }];
    });
}

function currentHash(filePath) {
  try {
    return hashFile(filePath);
  } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

function validateCandidateFiles({ destination, changes, expected }) {
  for (const change of changes) {
    if (!isManagedPath(change.path)) {
      throw new Error(`candidate contains unmanaged path ${change.path}`);
    }
    const expectedHash = expected[change.path] || null;
    const actualHash = currentHash(path.join(destination, change.path));
    if (actualHash !== expectedHash) {
      throw new Error(
        `candidate conflict for ${change.path}: expected ${expectedHash}, found ${actualHash}`,
      );
    }
  }
}

function applyCandidateFiles({ destination, work, changes, expected }) {
  validateCandidateFiles({ destination, changes, expected });
  for (const change of changes) {
    const destinationPath = path.join(destination, change.path);
    if (change.operation === "delete") {
      fs.unlinkSync(destinationPath);
      continue;
    }
    fs.mkdirSync(path.dirname(destinationPath), { recursive: true });
    fs.copyFileSync(path.join(work, change.path), destinationPath);
  }
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env || process.env,
      stdio: options.stdio || "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} exited code=${code} signal=${signal}`));
    });
  });
}

async function prepareWorkspace({ sourceRoot, workspaceRoot, corpusRoot }) {
  const baseline = path.join(workspaceRoot, "baseline");
  const work = path.join(workspaceRoot, "work");
  fs.rmSync(workspaceRoot, { recursive: true, force: true });
  fs.mkdirSync(baseline, { recursive: true });
  const common = [
    "-a",
    "--delete",
    "--exclude", "target/",
    "--exclude", "combat_research/",
    "--exclude", "live_traces/",
    "--exclude", "verification/corpus/",
    "--exclude", "python/.venv/",
  ];
  await run("rsync", [...common, `${path.join(sourceRoot, "simulator")}/`, `${path.join(baseline, "simulator")}/`]);
  fs.mkdirSync(path.join(baseline, "tools"), { recursive: true });
  await run(
    "rsync",
    [
      "-a",
      "--exclude", "session/",
      `${path.join(sourceRoot, "tools", "communication")}/`,
      `${path.join(baseline, "tools", "communication")}/`,
    ],
  );
  for (const name of ["AGENT_RULES.md", "PROJECT_OVERVIEW.md"]) {
    fs.copyFileSync(path.join(sourceRoot, name), path.join(baseline, name));
  }
  fs.mkdirSync(path.join(baseline, "docs"), { recursive: true });
  fs.copyFileSync(path.join(sourceRoot, "docs", "research.md"), path.join(baseline, "docs", "research.md"));
  fs.mkdirSync(path.join(baseline, "simulator", "verification"), { recursive: true });
  fs.symlinkSync(corpusRoot, path.join(baseline, "simulator", "verification", "corpus"), "dir");
  await run("cp", ["-a", "--reflink=auto", baseline, work]);
  return {
    baseline,
    work,
    baseline_files: snapshotFiles(baseline),
  };
}

module.exports = {
  applyCandidateFiles,
  candidateChanges,
  isManagedPath,
  prepareWorkspace,
  snapshotFiles,
  validateCandidateFiles,
};
