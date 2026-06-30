#!/usr/bin/env node

const assert = require("assert");
const fs = require("fs");
const path = require("path");

const REPO = path.resolve(__dirname, "..", "..");

function readScript(name) {
  return fs.readFileSync(path.join(__dirname, name), "utf8");
}

function normalize(script) {
  return script.replace(/\r\n/g, "\n");
}

function testRunAutoCollectDelegatesAndPreservesExitCode() {
  const script = normalize(readScript("run_auto_collect.cmd"));

  assert.match(script, /call "%REPO%\\tools\\communication\\run_guided_collect\.cmd" %\*/);
  assert.match(script, /set RESULT=%ERRORLEVEL%/);
  assert.match(script, /call "%REPO%\\tools\\communication\\run_guided_collect_status\.cmd"/);
  assert.match(script, /exit \/b %RESULT%/);
}

function testGuidedCollectorLauncherUsesStrictDefaults() {
  const script = normalize(readScript("run_guided_collect.cmd"));

  assert.match(script, /cd \/d "%REPO%\\simulator" \|\| exit \/b 1/);
  assert.match(script, /uv run python -m sts\.guided_collect/);
  assert.match(script, /--report-output target\\guided-collect\\latest\.json/);
  assert.match(script, /--archive-report-dir target\\guided-collect\\reports/);
  assert.match(script, /--fail-on-not-ok/);
  assert.match(script, /--preflight-timeout-seconds 30/);
  assert.match(script, / %\*/);
  assert.doesNotMatch(script, /--allow-file-bridge/);
}

function testStatusLauncherUsesGuidedStatusTool() {
  const script = normalize(readScript("run_guided_collect_status.cmd"));

  assert.match(script, /"%NODE%" "%REPO%\\tools\\communication\\guided_collect_status\.js" %\*/);
}

function testCommunicationChecksIncludeGuidedEntrypointCoverage() {
  const script = normalize(readScript("run_communication_checks.cmd"));

  assert.match(script, /guided_collect_status\.test\.js/);
  assert.match(script, /launcher_scripts\.test\.js/);
  assert.match(script, /trace_ui\\server\.test\.js/);
}

assert.ok(fs.existsSync(REPO));

testRunAutoCollectDelegatesAndPreservesExitCode();
testGuidedCollectorLauncherUsesStrictDefaults();
testStatusLauncherUsesGuidedStatusTool();
testCommunicationChecksIncludeGuidedEntrypointCoverage();

console.log("launcher_scripts tests passed");
