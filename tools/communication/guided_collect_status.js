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

function fileTimestamp(filePath, nowMs = Date.now()) {
  const stat = fs.statSync(filePath);
  return {
    modified_at: new Date(stat.mtimeMs).toISOString(),
    age_seconds: Math.max(0, (nowMs - stat.mtimeMs) / 1000),
  };
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

function summarizeStrictTraceValidation(value) {
  if (!value || typeof value !== "object") return null;
  const blocker = value.blocker && typeof value.blocker === "object" ? value.blocker : null;
  return {
    verified: Boolean(value.verified),
    reason: value.reason ?? null,
    stop_reason: value.stop_reason ?? null,
    steps: value.steps ?? null,
    final_phase: value.final_phase ?? null,
    blocker_reason: blocker ? blocker.reason ?? null : null,
    blocker_detail: blocker ? blocker.detail ?? null : null,
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
  const timestamp = fileTimestamp(reportPath);
  const trace = validateTrace(report.trace_path);
  const blocker = report.blocker && typeof report.blocker === "object" ? report.blocker : null;
  const selection = report.selection && typeof report.selection === "object" ? report.selection : null;
  const preflight = report.preflight && typeof report.preflight === "object" ? report.preflight : null;
  const strictTraceValidation = summarizeStrictTraceValidation(report.trace_validation);
  return {
    ok: Boolean(report.ok),
    report_path: reportPath,
    producer: report.producer ?? null,
    generated_at: report.generated_at ?? null,
    report_modified_at: timestamp.modified_at,
    report_age_seconds: timestamp.age_seconds,
    run_id: report.run_id ?? null,
    seed: report.seed ?? null,
    stop_reason: report.stop_reason ?? null,
    actions_sent: report.actions_sent ?? 0,
    elapsed_seconds: report.elapsed_seconds ?? null,
    bridge_step: report.bridge_step ?? null,
    bridge_state_id: report.bridge_state_id ?? null,
    tcp_control_available: Boolean(report.tcp_control_available),
    selection: selection
      ? {
        mode: selection.mode ?? null,
        selected_run_id: selection.selected_run_id ?? null,
        considered_count: selection.considered_count ?? null,
        candidate_count: selection.candidate_count ?? null,
        skipped_unsupported_count: Array.isArray(selection.skipped_unsupported)
          ? selection.skipped_unsupported.length
          : 0,
      }
      : null,
    preflight: preflight
      ? {
        ok: Boolean(preflight.ok),
        ages: preflight.ages || null,
        pending_command: preflight.pending_command || null,
        summary: preflight.summary || null,
        status: preflight.status || null,
      }
      : null,
    blocker: blocker
      ? {
        reason: blocker.reason ?? null,
        problems: blocker.problems || [],
        warnings: blocker.warnings || [],
        detail: blocker.detail ?? null,
      }
      : null,
    strict_trace_validation: strictTraceValidation,
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
  fileTimestamp,
  recentReports,
  summarizeStrictTraceValidation,
  validateTrace,
};
