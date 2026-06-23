#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { readTrace, validate } = require("./trace_tools");

const sessionDir = path.join(__dirname, "session");
const defaultReportPath = path.join(sessionDir, "harvest_report.json");

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function validatePath(filePath) {
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

function inspectHarvestReport(reportPath = defaultReportPath) {
  if (!fs.existsSync(reportPath)) {
    return {
      ok: false,
      report_path: reportPath,
      error: "harvest report not found",
    };
  }
  const report = readJson(reportPath);
  const raw = validatePath(report.trace_path);
  const validPrefix = validatePath(report.valid_prefix_path);
  const bestRun = validatePath(report.best_run?.extracted_path);
  return {
    ok: Boolean(bestRun.ok || validPrefix.ok || report.validation?.ok),
    report_path: reportPath,
    updated_at: report.updated_at,
    reason: report.reason,
    stale: report.stale || null,
    raw,
    valid_prefix: validPrefix,
    best_run: {
      ...bestRun,
      run_index: report.best_run?.run_index ?? null,
      start_step: report.best_run?.start_step ?? null,
      command: report.best_run?.command ?? null,
      encounters: report.best_run?.encounters || [],
    },
  };
}

if (require.main === module) {
  const reportPath = process.argv[2] || defaultReportPath;
  const result = inspectHarvestReport(reportPath);
  console.log(JSON.stringify(result, null, 2));
  process.exit(result.ok ? 0 : 1);
}

module.exports = {
  inspectHarvestReport,
  validatePath,
};
