#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { readTrace, validate } = require("./trace_tools");

const repoRoot = path.resolve(__dirname, "..", "..");
const defaultReportPath = path.join(repoRoot, "simulator", "target", "guided-collect", "latest.json");
const defaultArchiveDir = path.join(repoRoot, "simulator", "target", "guided-collect", "reports");

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function validateTrace(filePath) {
  if (!filePath) return { path: null, exists: false, ok: false };
  if (!fs.existsSync(filePath)) return { path: filePath, exists: false, ok: false };
  const result = validate(readTrace(filePath));
  return {
    path: filePath,
    exists: true,
    ok: result.ok,
    missing: result.missing,
    actions: result.summary.actions,
    max_floor: result.summary.max_floor,
    elite_rooms: result.summary.elite_rooms,
    boss_rooms: result.summary.boss_rooms,
    deaths: result.summary.deaths,
    terminal: result.summary.terminal,
    score: result.summary.coverage?.score ?? null,
  };
}

function inspectGuidedCollectReport(reportPath = defaultReportPath, archiveDir = defaultArchiveDir) {
  if (!fs.existsSync(reportPath)) {
    return {
      ok: false,
      report_path: reportPath,
      error: "guided collection report not found",
    };
  }
  const report = readJson(reportPath);
  const trace = validateTrace(report.trace_path);
  const blocker = report.blocker && typeof report.blocker === "object" ? report.blocker : null;
  return {
    ok: Boolean(report.ok),
    report_path: reportPath,
    run_id: report.run_id ?? null,
    seed: report.seed ?? null,
    stop_reason: report.stop_reason ?? null,
    actions_sent: report.actions_sent ?? 0,
    elapsed_seconds: report.elapsed_seconds ?? null,
    bridge_step: report.bridge_step ?? null,
    bridge_state_id: report.bridge_state_id ?? null,
    tcp_control_available: Boolean(report.tcp_control_available),
    blocker: blocker
      ? {
        reason: blocker.reason ?? null,
        problems: blocker.problems || [],
        warnings: blocker.warnings || [],
        detail: blocker.detail ?? null,
      }
      : null,
    trace,
    history_tail_count: Array.isArray(report.history_tail) ? report.history_tail.length : 0,
    recent_reports: recentReports(archiveDir),
  };
}

function recentReports(directory = defaultArchiveDir, limit = 5) {
  if (!directory || !fs.existsSync(directory)) return [];
  return fs.readdirSync(directory)
    .filter((name) => name.endsWith(".json"))
    .map((name) => {
      const filePath = path.join(directory, name);
      const stat = fs.statSync(filePath);
      return {
        path: filePath,
        name,
        modified_ms: stat.mtimeMs,
      };
    })
    .sort((left, right) => right.modified_ms - left.modified_ms)
    .slice(0, limit);
}

if (require.main === module) {
  const reportPath = process.argv[2] || defaultReportPath;
  const result = inspectGuidedCollectReport(reportPath);
  console.log(JSON.stringify(result, null, 2));
  process.exit(result.ok ? 0 : 1);
}

module.exports = {
  inspectGuidedCollectReport,
  recentReports,
  validateTrace,
};
