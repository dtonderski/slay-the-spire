#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const sessionDir = path.join(__dirname, "session");
const summaryPath = path.join(sessionDir, "summary.json");
const statusPath = path.join(sessionDir, "status.json");
const commandPath = path.join(sessionDir, "next_command.txt");
const harvestReportPath = path.join(sessionDir, "harvest_report.json");

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function fileAgeMs(filePath, now = Date.now()) {
  try {
    return now - fs.statSync(filePath).mtimeMs;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}

function checkPreflightFrom({
  summary,
  status,
  summaryAgeMs,
  statusAgeMs,
  commandExists,
  staleThresholdMs,
  harvestReport,
}) {
  const problems = [];
  const warnings = [];

  if (!summary) problems.push("missing session summary.json");
  if (!status) problems.push("missing session status.json");
  if (summaryAgeMs > staleThresholdMs && statusAgeMs > staleThresholdMs) {
    problems.push(`session files stale: summaryAgeMs=${Math.round(summaryAgeMs)} statusAgeMs=${Math.round(statusAgeMs)}`);
  }
  if (status?.status === "exited") {
    problems.push(`bridge exited: ${status.reason || "unknown"}`);
  }
  if (commandExists) {
    problems.push("next_command.txt already exists");
  }
  if (summary && summary.ready_for_command !== true) {
    warnings.push("latest summary is not ready_for_command");
  }
  if (status?.status === "sent" && summary && status.step > summary.step) {
    problems.push(`sent command step ${status.step} is newer than summary step ${summary.step}`);
  }
  if (summary?.available_commands && !summary.available_commands.includes("state")) {
    warnings.push("available_commands does not include state");
  }

  return {
    ok: problems.length === 0,
    problems,
    warnings,
    summary: summary
      ? {
          step: summary.step ?? null,
          client_pid: summary.client_pid ?? null,
          screen_type: summary.screen_type ?? null,
          floor: summary.floor ?? null,
          seed: summary.seed ?? null,
          ready_for_command: summary.ready_for_command ?? null,
          available_commands: summary.available_commands || [],
        }
      : null,
    status: status
      ? {
          step: status.step ?? null,
          client_pid: status.client_pid ?? null,
          status: status.status ?? null,
          trace_path: status.trace_path ?? null,
          command: status.command ?? null,
        }
      : null,
    harvest_report: harvestReport
      ? {
          updated_at: harvestReport.updated_at ?? null,
          reason: harvestReport.reason ?? null,
          best_run: harvestReport.best_run
            ? {
                actions: harvestReport.best_run.actions ?? null,
                max_floor: harvestReport.best_run.max_floor ?? null,
                elite_rooms: harvestReport.best_run.elite_rooms ?? 0,
                deaths: harvestReport.best_run.deaths ?? 0,
                extracted_path: harvestReport.best_run.extracted_path ?? null,
              }
            : null,
        }
      : null,
  };
}

function checkPreflight(options = {}) {
  const staleThresholdMs = options.staleThresholdMs ?? Number.parseInt(process.env.STS_PREFLIGHT_STALE_MS || "120000", 10);
  const now = options.now ?? Date.now();
  return checkPreflightFrom({
    summary: readJson(summaryPath),
    status: readJson(statusPath),
    summaryAgeMs: fileAgeMs(summaryPath, now),
    statusAgeMs: fileAgeMs(statusPath, now),
    commandExists: fs.existsSync(commandPath),
    staleThresholdMs,
    harvestReport: readJson(harvestReportPath),
  });
}

if (require.main === module) {
  const result = checkPreflight();
  console.log(JSON.stringify(result, null, 2));
  process.exit(result.ok ? 0 : 1);
}

module.exports = {
  checkPreflight,
  checkPreflightFrom,
};
