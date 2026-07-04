#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { readTrace, validate } = require("./trace_tools");

const repoRoot = path.resolve(__dirname, "..", "..");
const defaultReportPath = path.join(repoRoot, "simulator", "target", "guided-collect", "latest.json");
const defaultArchiveDir = path.join(repoRoot, "simulator", "target", "guided-collect", "reports");
const defaultSessionDir = path.join(repoRoot, "tools", "communication", "session");
const staleAfterSeconds = 120;

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

function readJsonIfExists(filePath) {
  if (!fs.existsSync(filePath)) return { missing: true };
  try {
    return readJson(filePath);
  } catch (error) {
    return { missing: false, error: error.message };
  }
}

function fileAgeSeconds(filePath, nowMs = Date.now()) {
  if (!fs.existsSync(filePath)) return null;
  return Math.max(0, (nowMs - fs.statSync(filePath).mtimeMs) / 1000);
}

function inspectCurrentBridgePreflight(sessionDir = defaultSessionDir, nowMs = Date.now()) {
  const statusPath = path.join(sessionDir, "status.json");
  const summaryPath = path.join(sessionDir, "summary.json");
  const commandPath = path.join(sessionDir, "next_command.txt");
  const commandMetaPath = path.join(sessionDir, "next_command.json");
  const status = readJsonIfExists(statusPath);
  const summary = readJsonIfExists(summaryPath);
  const statusAge = fileAgeSeconds(statusPath, nowMs);
  const summaryAge = fileAgeSeconds(summaryPath, nowMs);
  const commandExists = fs.existsSync(commandPath);
  const tcpPending = Boolean(status && status.pending_command);
  const commandMetaExists = fs.existsSync(commandMetaPath);
  const control = status && status.control && status.control.protocol === "tcp-jsonl"
    ? status.control
    : null;
  const problems = [];
  const warnings = [];

  if (status.missing) problems.push("missing session status.json");
  if (summary.missing) problems.push("missing session summary.json");
  if (summaryAge === null || summaryAge > staleAfterSeconds) problems.push("observed state summary is stale");
  if (status.status === "exited") problems.push(`bridge exited: ${status.reason || "unknown"}`);
  if (commandExists || tcpPending) problems.push("bridge command already pending");
  if (commandMetaExists && !commandExists) problems.push("next_command.json exists without next_command.txt");
  if (summary.ready_for_command !== true) warnings.push("latest summary is not ready_for_command");
  if (!control) warnings.push("TCP bridge control is not available; guided auto-collection will not send");
  const availableCommands = Array.isArray(summary.available_commands) ? summary.available_commands : [];
  if (control && availableCommands.some((command) => String(command).toLowerCase() !== "state") && summary.state_seq == null) {
    problems.push("TCP bridge summary is missing state_seq for guarded commands");
  }

  return {
    ok: problems.length === 0,
    problems,
    warnings,
    tcp_control_available: Boolean(control),
    control,
    ages: {
      status_age_seconds: statusAge,
      summary_age_seconds: summaryAge,
    },
    pending_command: {
      present: commandExists || tcpPending,
      transport: commandExists ? "file" : tcpPending ? "tcp-jsonl" : null,
    },
    summary: summary.missing ? null : {
      step: summary.step ?? null,
      state_seq: summary.state_seq ?? null,
      client_pid: summary.client_pid ?? null,
      screen_type: summary.screen_type ?? null,
      floor: summary.floor ?? null,
      seed: summary.seed ?? null,
      ready_for_command: summary.ready_for_command ?? null,
      available_commands: availableCommands,
    },
    status: status.missing ? null : {
      step: status.step ?? null,
      client_pid: status.client_pid ?? null,
      status: status.status ?? null,
      trace_path: status.trace_path ?? null,
      command: status.command ?? null,
    },
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
    control_actions: result.summary.control_actions,
    passive_polls: result.summary.passive_polls,
    command_accepts: result.summary.command_accepts,
    command_observed_timeouts: result.summary.command_observed_timeouts,
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

function summarizeBridgeClients(value) {
  if (!value || typeof value !== "object") return null;
  const clients = Array.isArray(value.clients) ? value.clients : [];
  const visibleClients = clients.filter((client) => client && (client.current || client.alive));
  const exitedCount = clients.length - visibleClients.length;
  return {
    alive_count: value.alive_count ?? visibleClients.filter((client) => client.alive).length,
    active_bridge_count: value.active_bridge_count ?? null,
    current_pid: value.current_pid ?? null,
    omitted_exited_count: Math.max(0, exitedCount),
    clients: visibleClients.map((client) => ({
      pid: client.pid ?? null,
      current: Boolean(client.current),
      alive: Boolean(client.alive),
      killable: client.killable ?? null,
      trace_paths: client.trace_paths || [],
    })),
  };
}

function inspectGuidedCollectReport(reportPath = defaultReportPath, archiveDir = defaultArchiveDir, options = {}) {
  const current_bridge_preflight = options.current_bridge_preflight === false
    ? null
    : inspectCurrentBridgePreflight(options.session_dir || defaultSessionDir);
  if (!fs.existsSync(reportPath)) {
    return {
      ok: false,
      report_path: reportPath,
      error: "guided collection report not found",
      current_bridge_preflight,
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
    current_bridge_preflight,
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
        bridge_clients: summarizeBridgeClients(preflight.bridge_clients),
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
  inspectCurrentBridgePreflight,
  fileTimestamp,
  recentReports,
  summarizeStrictTraceValidation,
  validateTrace,
};
