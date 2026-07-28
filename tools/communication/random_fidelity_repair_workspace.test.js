#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const {
  applyCandidateFiles,
  candidateChanges,
  snapshotFiles,
} = require("./random_fidelity_repair_workspace");

const root = fs.mkdtempSync(path.join(os.tmpdir(), "sts-repair-workspace-"));
try {
  const baseline = path.join(root, "baseline");
  const work = path.join(root, "work");
  const destination = path.join(root, "destination");
  for (const directory of [baseline, work, destination]) {
    fs.mkdirSync(path.join(directory, "simulator", "crates"), { recursive: true });
  }
  fs.writeFileSync(path.join(baseline, "simulator", "crates", "same.rs"), "same\n");
  fs.writeFileSync(path.join(baseline, "simulator", "crates", "changed.rs"), "old\n");
  fs.writeFileSync(path.join(baseline, "simulator", "crates", "deleted.rs"), "delete\n");
  fs.cpSync(baseline, work, { recursive: true });
  fs.cpSync(baseline, destination, { recursive: true });
  fs.writeFileSync(path.join(work, "simulator", "crates", "changed.rs"), "new\n");
  fs.unlinkSync(path.join(work, "simulator", "crates", "deleted.rs"));
  fs.writeFileSync(path.join(work, "simulator", "crates", "added.rs"), "add\n");

  const before = snapshotFiles(baseline);
  const after = snapshotFiles(work);
  const changes = candidateChanges(before, after);
  assert.deepStrictEqual(
    changes.map((change) => [change.path, change.operation]),
    [
      ["simulator/crates/added.rs", "add"],
      ["simulator/crates/changed.rs", "modify"],
      ["simulator/crates/deleted.rs", "delete"],
    ],
  );

  applyCandidateFiles({ destination, work, changes, expected: before });
  assert.strictEqual(
    fs.readFileSync(path.join(destination, "simulator", "crates", "changed.rs"), "utf8"),
    "new\n",
  );
  assert.strictEqual(fs.existsSync(path.join(destination, "simulator", "crates", "deleted.rs")), false);
  assert.strictEqual(
    fs.readFileSync(path.join(destination, "simulator", "crates", "added.rs"), "utf8"),
    "add\n",
  );

  fs.writeFileSync(path.join(destination, "simulator", "crates", "changed.rs"), "conflict\n");
  assert.throws(
    () => applyCandidateFiles({ destination, work, changes, expected: before }),
    /candidate conflict/,
  );
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}

console.log("random fidelity repair workspace tests passed");
