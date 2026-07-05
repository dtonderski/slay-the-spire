import "./styles.css";

type Character = "ironclad";
type RunSeed = { external: string } | { numeric: number };
type SessionLifecycle =
  | "not_attached"
  | "attached"
  | "recording"
  | "fidelity_ok"
  | "fidelity_lost"
  | "blocked"
  | "ended";

interface RunConfig {
  character: Character;
  ascension: number;
  seed: RunSeed;
}

interface BridgeStatus {
  id: string;
  process_id?: number | null;
  client_id?: string | null;
  connected: boolean;
  last_heartbeat_ms?: number | null;
}

interface BridgesResponse {
  bridges: BridgeStatus[];
}

interface SessionListItem {
  session_id: string;
  lifecycle: SessionLifecycle;
}

interface SessionsResponse {
  sessions: SessionListItem[];
}

interface FidelityStatus {
  kind: string;
  first_divergent_step?: number | null;
  compact_diff?: string[];
  message?: string | null;
}

interface LegalAction {
  id: string;
  kind: string;
  label: string;
  enabled: boolean;
  command?: unknown;
  disabled_reason?: string | null;
}

interface LiveState {
  sequence: number;
  phase: string;
  legal_actions: LegalAction[];
  raw?: {
    summary?: {
      potions?: Array<{ index?: number; name?: string; id?: string; can_use?: boolean }>;
    };
  };
}

interface BlockedState {
  reason_code: string;
  message: string;
}

interface SessionSnapshot {
  session_id: string;
  bridge_id: string;
  lifecycle: SessionLifecycle;
  trace_path: string;
  run_config?: RunConfig | null;
  latest_state?: LiveState | null;
  fidelity: FidelityStatus;
  blocked?: BlockedState | null;
  automation: AutomationJobSnapshot;
}

type AutomationPolicy = "fake_play_first_card" | "greedy_search" | "beam_search";

interface AutomationConfig {
  policy: AutomationPolicy;
  depth: number;
  width: number;
  allowed_potion_slots: number[];
  auto_action_limit: number;
}

interface AutomationPlannedAction {
  action_id: string;
  kind: string;
  label: string;
  source_sequence: number;
  command?: string | null;
  planner_action: string;
}

interface AutomationPlanSnapshot {
  actions: AutomationPlannedAction[];
  played_actions?: number;
  predicted_final_hp?: number | null;
  predicted_monster_hp?: number | null;
  value?: number | null;
  nodes: number;
  terminal_reason?: string | null;
}

interface AutomationJobSnapshot {
  state: string;
  policy: AutomationPolicy;
  config: AutomationConfig;
  planned_action?: AutomationPlannedAction | null;
  plan?: AutomationPlanSnapshot | null;
  executed_actions?: AutomationPlannedAction[];
  blocked?: BlockedState | null;
  last_message?: string | null;
}

interface PendingCommand {
  sessionId: string;
  sourceSequence: number;
  actionId: string;
  label: string;
  startedAt: number;
  timedOut: boolean;
}

interface StartPayload {
  bridge_id: string;
  config: RunConfig;
}

interface ErrorPayload {
  error?: {
    message?: string;
  };
}

type ActionGroupKey =
  | "card"
  | "discard"
  | "potion"
  | "turn"
  | "reward"
  | "choice"
  | "navigation"
  | "utility"
  | "operator"
  | "other";

let currentSession: SessionSnapshot | null = null;
let bridgeStatuses: BridgeStatus[] = [];
let sessionStatuses: SessionListItem[] = [];
let automationDrafts = new Map<string, AutomationConfig>();
let currentSessionRenderedAt = 0;
let startupSessionLoaded = false;
let bridgeRefreshInFlight = false;
let sessionRefreshInFlight = false;
let activeSessionRefreshTimer: number | null = null;
let pendingCommand: PendingCommand | null = null;
let pendingCommandTimer: number | null = null;
let renderedActionsKey: string | null = null;
let renderedAutomationSummaryKey: string | null = null;
const BRIDGE_REFRESH_MS = 1000;
const SESSION_REFRESH_MS = 2000;
const ACTIVE_SESSION_REFRESH_MS = 150;
const COMMAND_PENDING_TIMEOUT_MS = 3500;

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`missing #${id}`);
  }
  return element as T;
}

function formatError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error || "Unknown error");
  return message
    .replace(/^Error:\s*/, "")
    .replace(/^bridge error:\s*/i, "Bridge: ");
}

function notifyError(error: unknown): void {
  const container = byId<HTMLDivElement>("notifications");
  const notification = document.createElement("section");
  notification.className = "notification";

  const content = document.createElement("div");
  const title = document.createElement("div");
  title.className = "notification-title";
  title.textContent = "Action failed";
  const message = document.createElement("div");
  message.className = "notification-message";
  message.textContent = formatError(error);
  content.append(title, message);

  const close = document.createElement("button");
  close.type = "button";
  close.textContent = "x";
  close.title = "Dismiss";
  close.addEventListener("click", () => notification.remove());

  notification.append(content, close);
  container.appendChild(notification);
  window.setTimeout(() => notification.remove(), 12000);
}

function run(task: () => Promise<void>): void {
  task().catch(notifyError);
}

async function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(path, {
    headers: { "content-type": "application/json" },
    ...options
  });
  const body = (await response.json()) as ErrorPayload | T;
  if (!response.ok) {
    const payload = body as ErrorPayload;
    const message = payload.error?.message || response.statusText;
    throw new Error(message);
  }
  return body as T;
}

async function refreshBridges(): Promise<void> {
  const data = await api<BridgesResponse>("/bridges");
  const bridge = byId<HTMLSelectElement>("bridge");
  const selected = bridge.value;
  bridgeStatuses = data.bridges;
  bridge.replaceChildren();
  for (const item of bridgeStatuses) {
    const option = document.createElement("option");
    option.value = item.id;
    option.textContent = bridgeOptionLabel(item);
    bridge.appendChild(option);
  }
  if (bridgeStatuses.length === 0) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "No active bridges";
    bridge.appendChild(option);
  } else if (selected && bridgeStatuses.some((item) => item.id === selected)) {
    bridge.value = selected;
  }
  syncBridgeControls();
}

function bridgeOptionLabel(bridge: BridgeStatus): string {
  const pid = bridge.process_id ? ` pid ${bridge.process_id}` : "";
  const heartbeat = bridge.last_heartbeat_ms == null ? "" : ` ${formatAge(bridge.last_heartbeat_ms)}`;
  if (bridge.id.startsWith("local-process-")) {
    return `${bridge.client_id || "local bridge process"}${pid}${heartbeat} (kill only)`;
  }
  if (bridge.connected) {
    return `${bridge.id}${pid}${heartbeat}`;
  }
  return `${bridge.id}${pid}${heartbeat} (not connected)`;
}

async function refreshSessions(): Promise<void> {
  const data = await api<SessionsResponse>("/sessions");
  const session = byId<HTMLSelectElement>("session");
  const selected = currentSession?.session_id || session.value;
  sessionStatuses = [...data.sessions].sort(compareSessionsNewestFirst);
  session.replaceChildren();
  for (const item of sessionStatuses) {
    const option = document.createElement("option");
    option.value = item.session_id;
    option.textContent = `${item.session_id} | ${item.lifecycle}`;
    session.appendChild(option);
  }
  const selectedExists = selected && sessionStatuses.some((item) => item.session_id === selected);
  if (selectedExists) {
    session.value = selected;
  } else {
    const latest = sessionStatuses[0];
    if (latest) {
      session.value = latest.session_id;
    }
  }
  syncBridgeControls();
}

function compareSessionsNewestFirst(left: SessionListItem, right: SessionListItem): number {
  const leftNumber = sessionNumber(left.session_id);
  const rightNumber = sessionNumber(right.session_id);
  if (leftNumber !== rightNumber) {
    return rightNumber - leftNumber;
  }
  return right.session_id.localeCompare(left.session_id);
}

function sessionNumber(sessionId: string): number {
  const match = /^session-(\d+)$/.exec(sessionId);
  return match ? Number(match[1]) : Number.NEGATIVE_INFINITY;
}

async function loadLatestSessionOnStartup(): Promise<void> {
  const latest = sessionStatuses[0];
  if (startupSessionLoaded || currentSession || !latest) {
    return;
  }
  startupSessionLoaded = true;
  const latestSessionId = latest.session_id;
  byId<HTMLSelectElement>("session").value = latestSessionId;
  renderSession(await api<SessionSnapshot>(`/sessions/${latestSessionId}`));
}

function renderSession(session: SessionSnapshot): void {
  session.automation ??= defaultAutomation();
  reconcilePendingCommand(session);
  currentSession = session;
  currentSessionRenderedAt = Date.now();
  byId<HTMLButtonElement>("request").disabled = false;
  byId<HTMLButtonElement>("abandon").disabled =
    !session.latest_state || session.latest_state.phase === "menu";
  byId<HTMLDivElement>("status").textContent = statusSummary(session);
  byId<HTMLElement>("lifecycle").textContent = session.lifecycle;
  byId<HTMLElement>("trace").textContent = session.trace_path;
  renderFidelityChip(session.fidelity.kind);
  byId<HTMLElement>("reason").textContent = healthReason(session);
  byId<HTMLElement>("first-divergent").textContent =
    session.fidelity.first_divergent_step == null
      ? "-"
      : `step ${session.fidelity.first_divergent_step}`;
  byId<HTMLElement>("phase").textContent = session.latest_state?.phase || "-";
  updateStateFreshness();
  byId<HTMLPreElement>("diff").textContent = (session.fidelity.compact_diff || []).join("\n");
  renderSessionAlert(session);
  renderCommandStatus();
  renderActions(session.latest_state?.legal_actions || []);
  renderAutomation(session);
  syncBridgeControls();
  scheduleActiveSessionRefresh(session);
  refreshSessions().catch(console.error);
}

function scheduleActiveSessionRefresh(session: SessionSnapshot): void {
  if (!automationNeedsFastRefresh(session.automation) && !isCommandPending()) {
    clearActiveSessionRefresh();
    return;
  }
  if (activeSessionRefreshTimer !== null) return;
  activeSessionRefreshTimer = window.setTimeout(() => {
    activeSessionRefreshTimer = null;
    refreshCurrentSessionIfIdle().catch(console.error);
  }, ACTIVE_SESSION_REFRESH_MS);
}

function isCommandPending(): boolean {
  return pendingCommand !== null;
}

function reconcilePendingCommand(session: SessionSnapshot): void {
  if (!pendingCommand) return;
  if (
    pendingCommand.sessionId !== session.session_id ||
    session.blocked ||
    (session.latest_state?.sequence ?? 0) > pendingCommand.sourceSequence
  ) {
    clearPendingCommand();
  }
}

function setPendingCommand(action: LegalAction): void {
  if (!currentSession) return;
  clearPendingCommand();
  pendingCommand = {
    sessionId: currentSession.session_id,
    sourceSequence: currentSession.latest_state?.sequence ?? 0,
    actionId: action.id,
    label: action.label,
    startedAt: Date.now(),
    timedOut: false
  };
  pendingCommandTimer = window.setTimeout(() => {
    if (!pendingCommand) return;
    pendingCommand.timedOut = true;
    renderPendingCommandState();
  }, COMMAND_PENDING_TIMEOUT_MS);
  renderPendingCommandState();
  scheduleActiveSessionRefresh(currentSession);
}

function clearPendingCommand(): void {
  if (pendingCommandTimer !== null) {
    window.clearTimeout(pendingCommandTimer);
    pendingCommandTimer = null;
  }
  pendingCommand = null;
}

function renderPendingCommandState(): void {
  renderCommandStatus();
  renderActions(currentSession?.latest_state?.legal_actions || []);
  if (currentSession) {
    renderAutomation(currentSession);
  }
}

function renderCommandStatus(): void {
  const status = byId<HTMLDivElement>("command-status");
  if (!pendingCommand) {
    status.hidden = true;
    status.textContent = "";
    status.className = "command-status";
    return;
  }
  status.hidden = false;
  status.className = `command-status ${pendingCommand.timedOut ? "command-status-timeout" : ""}`;
  status.textContent = pendingCommand.timedOut
    ? `Still waiting for the game state after ${pendingCommand.label}. Request state is available.`
    : `Waiting for game state after ${pendingCommand.label}...`;
}

function clearActiveSessionRefresh(): void {
  if (activeSessionRefreshTimer === null) return;
  window.clearTimeout(activeSessionRefreshTimer);
  activeSessionRefreshTimer = null;
}

function automationNeedsFastRefresh(automation: AutomationJobSnapshot): boolean {
  return [
    "auto_playing",
    "planning",
    "waiting_for_fidelity",
    "ready_to_send",
    "sending_action",
    "waiting_for_observed_state",
    "verifying_transition"
  ].includes(automation.state);
}

function renderFidelityChip(kind: string): void {
  const fidelity = byId<HTMLElement>("fidelity");
  fidelity.textContent = kind;
  fidelity.className = `fidelity-chip ${fidelityClass(kind)}`;
}

function fidelityClass(kind: string): string {
  if (kind === "ok") return "fidelity-ok";
  if (kind === "lost") return "fidelity-lost";
  return "fidelity-unverified";
}

function statusSummary(session: SessionSnapshot): string {
  const parts = [sessionStatusLabel(session)];
  if (session.latest_state) {
    parts.push(`${session.latest_state.phase} state #${session.latest_state.sequence}`);
  }
  if (session.blocked) {
    parts.push(`blocked: ${session.blocked.reason_code}`);
  }
  return parts.join(" | ");
}

function sessionStatusLabel(session: SessionSnapshot): string {
  if (session.lifecycle === "fidelity_ok" && session.fidelity.kind === "ok") {
    return "fidelity ok";
  }
  if (session.lifecycle === "fidelity_lost" && session.fidelity.kind === "lost") {
    return "fidelity lost";
  }
  return session.lifecycle;
}

function healthReason(session: SessionSnapshot): string {
  if (session.blocked) {
    return `${session.blocked.reason_code}: ${session.blocked.message}`;
  }
  return session.fidelity.message || "-";
}

function updateStateFreshness(): void {
  byId<HTMLElement>("state-freshness").textContent = stateFreshness();
}

function stateFreshness(): string {
  if (!currentSession?.latest_state) {
    return "-";
  }
  return `#${currentSession.latest_state.sequence}, refreshed ${formatAge(Date.now() - currentSessionRenderedAt)}`;
}

function renderSessionAlert(session: SessionSnapshot): void {
  const alert = byId<HTMLElement>("session-alert");
  const selected = selectedBridge();
  const messages: string[] = [];
  if (selected && !selected.connected) {
    messages.push(`Bridge ${selected.id} is not connected; live commands will not be available.`);
  }
  if (!session.latest_state) {
    messages.push("No live state is loaded for this session.");
  } else if (session.latest_state.legal_actions.length === 0) {
    messages.push(`No legal actions are available for this ${session.latest_state.phase} state.`);
  }
  if (session.blocked) {
    messages.push(`${session.blocked.reason_code}: ${session.blocked.message}`);
  }
  if (messages.length === 0) {
    alert.hidden = true;
    alert.textContent = "";
    return;
  }
  alert.hidden = false;
  alert.textContent = messages.join(" ");
}

function formatAge(ageMs: number): string {
  if (ageMs < 1000) return "just now";
  const seconds = Math.round(ageMs / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  return `${minutes}m ago`;
}

function renderActions(actions: LegalAction[]): void {
  const container = byId<HTMLDivElement>("actions");
  const renderKey = actionsRenderKey(actions);
  if (renderKey === renderedActionsKey) {
    return;
  }
  const scrollTop = container.scrollTop;
  const scrollLeft = container.scrollLeft;
  const pageScrollX = window.scrollX;
  const pageScrollY = window.scrollY;
  renderedActionsKey = renderKey;
  container.replaceChildren();
  if (actions.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.textContent = currentSession?.latest_state
      ? "No legal actions were reported for the current state."
      : "Open a trace or start a run to populate legal actions.";
    container.appendChild(empty);
    restoreActionScroll(container, scrollTop, scrollLeft, pageScrollX, pageScrollY);
    return;
  }
  for (const group of groupedActions(actions)) {
    const section = document.createElement("section");
    section.className = "action-group";
    section.dataset.groupKey = group.key;

    const heading = document.createElement("h3");
    heading.textContent = group.label;
    section.appendChild(heading);

    const row = document.createElement("div");
    row.className = "actions-row";
    for (const action of group.actions) {
      const button = document.createElement("button");
      button.textContent = action.label;
      button.disabled = !action.enabled || !currentSession || actionBlockedByPendingCommand(action);
      button.title = action.disabled_reason || action.kind;
      if (pendingCommand?.actionId === action.id) {
        button.classList.add("sending-action");
        button.title = pendingCommand.timedOut
          ? "Still waiting for a newer game state"
          : "Waiting for a newer game state";
      }
      if (currentPlannedActionId() === action.id) {
        button.classList.add("planned-action");
        button.title = `Next planned action: ${button.title}`;
      }
      button.addEventListener("click", () => {
        run(async () => {
          if (!currentSession) {
            throw new Error("No session selected");
          }
          if (action.kind !== "request_state") {
            setPendingCommand(action);
          }
          try {
            const session = await api<SessionSnapshot>(
              `/sessions/${currentSession.session_id}/actions/${action.id}`,
              { method: "POST", body: "{}" }
            );
            renderSession(session);
          } catch (error) {
            clearPendingCommand();
            renderPendingCommandState();
            throw error;
          }
        });
      });
      row.appendChild(button);
    }
    section.appendChild(row);
    container.appendChild(section);
  }
  restoreActionScroll(container, scrollTop, scrollLeft, pageScrollX, pageScrollY);
}

function actionsRenderKey(actions: LegalAction[]): string {
  const pendingKey = pendingCommand
    ? `${pendingCommand.sessionId}:${pendingCommand.actionId}:${pendingCommand.timedOut ? "timed-out" : "waiting"}`
    : "none";
  return JSON.stringify({
    session: currentSession?.session_id ?? null,
    sequence: currentSession?.latest_state?.sequence ?? null,
    planned: currentPlannedActionId(),
    pending: pendingKey,
    actions: actions.map((action) => ({
      id: action.id,
      kind: action.kind,
      label: action.label,
      enabled: action.enabled,
      disabled_reason: action.disabled_reason ?? null,
      command: action.command ?? null
    }))
  });
}

function restoreActionScroll(
  container: HTMLElement,
  scrollTop: number,
  scrollLeft: number,
  pageScrollX: number,
  pageScrollY: number
): void {
  container.scrollTop = Math.min(scrollTop, container.scrollHeight);
  container.scrollLeft = Math.min(scrollLeft, container.scrollWidth);
  if (window.scrollX !== pageScrollX || window.scrollY !== pageScrollY) {
    window.scrollTo(pageScrollX, pageScrollY);
  }
}

function actionBlockedByPendingCommand(action: LegalAction): boolean {
  if (!pendingCommand) return false;
  if (action.kind === "request_state") return false;
  return true;
}

function currentPlannedActionId(): string | null {
  const automation = currentSession?.automation;
  if (!automation) return null;
  if (automation.planned_action?.action_id) {
    return automation.planned_action.action_id;
  }
  const plan = automation.plan;
  if (!plan) return null;
  const action = plan.actions[plan.played_actions || 0];
  if (!action || action.action_id === "future" || action.action_id === "unmapped") {
    return null;
  }
  return action.action_id;
}

function renderAutomation(session: SessionSnapshot): void {
  const automation = session.automation ?? defaultAutomation();
  const config = automationConfigForSession(session);
  byId<HTMLSelectElement>("automation-policy").value = config.policy;
  byId<HTMLInputElement>("automation-depth").value = String(config.depth);
  byId<HTMLInputElement>("automation-width").value = String(config.width);
  byId<HTMLInputElement>("automation-limit").value = String(config.auto_action_limit);
  renderPotionSlots(session, config);
  const canAutomate = Boolean(session.latest_state && session.lifecycle !== "ended");
  const pending = isCommandPending();
  byId<HTMLButtonElement>("automation-plan").disabled = !canAutomate || pending;
  byId<HTMLButtonElement>("automation-run-one").disabled = !canAutomate || pending;
  byId<HTMLButtonElement>("automation-auto-play").disabled = !canAutomate || pending;
  byId<HTMLButtonElement>("automation-resume").disabled = !canAutomate || pending;
  byId<HTMLButtonElement>("automation-pause").disabled = !canAutomate;
  byId<HTMLButtonElement>("automation-cancel").disabled = !canAutomate;
  renderAutomationSummary(automation);
}

function renderAutomationSummary(automation: AutomationJobSnapshot): void {
  const summaryKey = automationSummaryRenderKey(automation);
  if (summaryKey === renderedAutomationSummaryKey) {
    return;
  }
  renderedAutomationSummaryKey = summaryKey;
  const summary = byId<HTMLDivElement>("automation-summary");
  const previousPlanList = summary.querySelector<HTMLDivElement>(".plan-list");
  const previousPlanKey = previousPlanList?.dataset.planKey;
  const previousPlanScrollTop = previousPlanList?.scrollTop ?? 0;
  summary.replaceChildren();

  const statusRow = document.createElement("div");
  statusRow.className = "automation-status-row";

  const state = document.createElement("span");
  state.className = `automation-state state-${automation.state.replace(/_/g, "-")}`;
  state.textContent = automation.state;
  statusRow.appendChild(state);

  if (automation.last_message) {
    const message = document.createElement("span");
    message.className = "automation-message";
    message.textContent = automation.last_message;
    statusRow.appendChild(message);
  }
  summary.appendChild(statusRow);

  if (automation.blocked) {
    const blocked = document.createElement("div");
    blocked.className = "automation-blocked";
    blocked.textContent = `${automation.blocked.reason_code}: ${automation.blocked.message}`;
    summary.appendChild(blocked);
  }

  const executedActions = automation.executed_actions || [];
  if (automation.plan) {
    const hp = automation.plan.predicted_final_hp ?? "-";
    const monsterHp = automation.plan.predicted_monster_hp ?? "-";
    const terminal = automation.plan.terminal_reason || "depth";
    const metrics = document.createElement("div");
    metrics.className = "plan-metrics";
    for (const [label, value] of [
      ["Final HP", hp],
      ["Monsters", monsterHp],
      ["Nodes", automation.plan.nodes],
      ["Result", terminal]
    ]) {
      const metric = document.createElement("span");
      metric.className = "plan-metric";
      metric.textContent = `${label} ${value}`;
      metrics.appendChild(metric);
    }
    summary.appendChild(metrics);

    const playedActions = automation.plan.played_actions || 0;
    const futureActions = automation.plan.actions.slice(playedActions);
    renderPlanList(summary, [...executedActions, ...futureActions], executedActions.length, automation, previousPlanKey, previousPlanScrollTop);
  } else if (executedActions.length > 0) {
    renderPlanList(summary, executedActions, executedActions.length, automation, previousPlanKey, previousPlanScrollTop);
  }

  if (!automation.last_message && !automation.plan && !automation.blocked && executedActions.length === 0) {
    const empty = document.createElement("div");
    empty.className = "automation-empty";
    empty.textContent = "No plan";
    summary.appendChild(empty);
  }
}

function automationSummaryRenderKey(automation: AutomationJobSnapshot): string {
  return JSON.stringify({
    session: currentSession?.session_id ?? null,
    state: automation.state,
    last_message: automation.last_message ?? null,
    blocked: automation.blocked ?? null,
    planned_action: automation.planned_action ?? null,
    plan: automation.plan ?? null,
    executed_actions: automation.executed_actions ?? []
  });
}

function renderPlanList(
  summary: HTMLDivElement,
  actions: AutomationPlannedAction[],
  executedCount: number,
  automation: AutomationJobSnapshot,
  previousPlanKey: string | undefined,
  previousPlanScrollTop: number
): void {
  if (actions.length === 0) return;
  const list = document.createElement("div");
  list.className = "plan-list";
  list.setAttribute("aria-label", "Planned action sequence");
  const planKey = planScrollKey(automation);
  list.dataset.planKey = planKey;
  actions.forEach((action, index) => {
    const step = document.createElement("span");
    step.className = "plan-step";
    if (index < executedCount) {
      step.classList.add("played-plan-step");
    }
    if (index === executedCount) {
      step.classList.add("next-plan-step");
    }
    if (action.action_id === "unmapped") {
      step.classList.add("unmapped-plan-step");
    }
    const ordinal = document.createElement("span");
    ordinal.className = "plan-step-index";
    ordinal.textContent = String(index + 1);
    const label = document.createElement("span");
    label.className = "plan-step-label";
    label.textContent = action.label;
    step.append(ordinal, label);
    step.title = action.command || action.planner_action;
    list.appendChild(step);
  });
  summary.appendChild(list);
  if (previousPlanKey === planKey) {
    list.scrollTop = previousPlanScrollTop;
  }
}

function planScrollKey(automation: AutomationJobSnapshot): string {
  const executedKey = (automation.executed_actions || [])
    .map((action) => `${action.kind}:${action.command || ""}:${action.planner_action}`)
    .join("|");
  const plan = automation.plan;
  if (!plan) return `history:${executedKey}`;
  return `history:${executedKey};plan:${plan.nodes}:${plan.played_actions || 0}:${plan.actions
    .map((action) => `${action.kind}:${action.command || ""}:${action.planner_action}`)
    .join("|")}`;
}

function defaultAutomation(): AutomationJobSnapshot {
  return {
    state: "idle",
    policy: "beam_search",
    config: {
      policy: "beam_search",
      depth: 50,
      width: 50,
      allowed_potion_slots: [],
      auto_action_limit: 80
    },
    planned_action: null,
    plan: null,
    executed_actions: [],
    blocked: null,
    last_message: null
  };
}

function renderPotionSlots(session: SessionSnapshot, config: AutomationConfig): void {
  const container = byId<HTMLDivElement>("automation-potions");
  const selected = new Set(config.allowed_potion_slots);
  const potions = session.latest_state?.raw?.summary?.potions || [];
  container.replaceChildren();
  if (potions.length === 0) {
    container.textContent = "No usable potions";
    return;
  }
  for (const potion of potions) {
    const slot = potion.index ?? 0;
    const label = document.createElement("label");
    label.className = "potion-slot";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.value = String(slot);
    checkbox.checked = selected.has(slot);
    checkbox.disabled = potion.can_use === false;
    label.append(checkbox, document.createTextNode(potion.name || potion.id || `Slot ${slot}`));
    container.appendChild(label);
  }
}

function automationConfigForSession(session: SessionSnapshot): AutomationConfig {
  return cloneAutomationConfig(
    automationDrafts.get(session.session_id) || session.automation.config
  );
}

function cloneAutomationConfig(config: AutomationConfig): AutomationConfig {
  return {
    policy: config.policy,
    depth: config.depth,
    width: config.width,
    allowed_potion_slots: [...config.allowed_potion_slots],
    auto_action_limit: config.auto_action_limit
  };
}

const GROUP_LABELS: Record<ActionGroupKey, string> = {
  card: "Cards",
  discard: "Discard",
  potion: "Potions",
  turn: "Turn",
  reward: "Rewards",
  choice: "Choices",
  navigation: "Navigation",
  utility: "Utility",
  operator: "Operator",
  other: "Other"
};

const GROUP_ORDER: ActionGroupKey[] = [
  "card",
  "discard",
  "potion",
  "turn",
  "reward",
  "choice",
  "navigation",
  "utility",
  "operator",
  "other"
];

function groupedActions(actions: LegalAction[]): Array<{
  key: ActionGroupKey;
  label: string;
  actions: LegalAction[];
}> {
  const groups = new Map<ActionGroupKey, LegalAction[]>();
  for (const action of actions) {
    const key = actionGroupKey(action);
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key)?.push(action);
  }
  return GROUP_ORDER
    .filter((key) => groups.has(key))
    .map((key) => ({ key, label: GROUP_LABELS[key], actions: groups.get(key) || [] }));
}

function actionGroupKey(action: LegalAction): ActionGroupKey {
  const kind = action.kind || "";
  const label = action.label || "";
  if (kind === "abandon_run") return "operator";
  if (kind.includes("discard") || /^discard\b/i.test(label)) return "discard";
  if (kind === "play_card") return "card";
  if (kind.includes("potion")) return "potion";
  if (kind === "end_turn") return "turn";
  if (kind.includes("reward")) return "reward";
  if (kind.includes("choose") || kind === "confirm" || kind === "event_choice") return "choice";
  if (
    kind.includes("map") ||
    kind === "open_chest" ||
    kind === "rest_site" ||
    kind.includes("shop")
  ) {
    return "navigation";
  }
  if (kind === "request_state") return "utility";
  return "other";
}

function runConfigFromControls(): RunConfig {
  const seedValue = byId<HTMLInputElement>("seed").value.trim();
  const numeric = /^-?\d+$/.test(seedValue);
  return {
    character: byId<HTMLSelectElement>("character").value as Character,
    ascension: Number(byId<HTMLInputElement>("ascension").value),
    seed: numeric ? { numeric: Number(seedValue) } : { external: seedValue }
  };
}

function startPayload(): StartPayload {
  return {
    bridge_id: byId<HTMLSelectElement>("bridge").value,
    config: runConfigFromControls()
  };
}

async function startRun(): Promise<void> {
  const payload = startPayload();
  const session = await api<SessionSnapshot>("/sessions/start", {
    method: "POST",
    body: JSON.stringify(payload)
  });
  renderSession(session);
}

async function loadSelectedSession(): Promise<void> {
  const sessionId = byId<HTMLSelectElement>("session").value;
  if (!sessionId) return;
  renderSession(await api<SessionSnapshot>(`/sessions/${sessionId}`));
}

async function refreshCurrentSession(): Promise<void> {
  if (!currentSession) return;
  const sessionId = currentSession.session_id;
  const session = await api<SessionSnapshot>(`/sessions/${sessionId}`);
  if (currentSession?.session_id === sessionId) {
    renderSession(session);
  }
}

async function refreshBridgesIfIdle(): Promise<void> {
  if (bridgeRefreshInFlight) return;
  bridgeRefreshInFlight = true;
  try {
    await refreshBridges();
  } finally {
    bridgeRefreshInFlight = false;
  }
}

async function refreshCurrentSessionIfIdle(): Promise<void> {
  if (sessionRefreshInFlight) return;
  sessionRefreshInFlight = true;
  try {
    await refreshCurrentSession();
  } finally {
    sessionRefreshInFlight = false;
  }
}

async function requestState(): Promise<void> {
  if (!currentSession) return;
  renderSession(await api<SessionSnapshot>(`/sessions/${currentSession.session_id}/request-state`, {
    method: "POST",
    body: "{}"
  }));
}

async function abandonRun(): Promise<void> {
  if (!currentSession) return;
  if (!window.confirm(`Abandon run for ${currentSession.session_id}?`)) return;
  renderSession(await api<SessionSnapshot>(`/sessions/${currentSession.session_id}/abandon`, {
    method: "POST",
    body: "{}"
  }));
}

function automationConfigFromControls(): AutomationConfig {
  const potionSlots = Array.from(
    byId<HTMLDivElement>("automation-potions").querySelectorAll<HTMLInputElement>("input:checked")
  ).map((input) => Number(input.value));
  return {
    policy: byId<HTMLSelectElement>("automation-policy").value as AutomationPolicy,
    depth: Number(byId<HTMLInputElement>("automation-depth").value),
    width: Number(byId<HTMLInputElement>("automation-width").value),
    allowed_potion_slots: potionSlots,
    auto_action_limit: Number(byId<HTMLInputElement>("automation-limit").value)
  };
}

async function configureAutomation(): Promise<void> {
  if (!currentSession) return;
  const sessionId = currentSession.session_id;
  const config = automationConfigFromControls();
  automationDrafts.set(sessionId, cloneAutomationConfig(config));
  const automation = await api<AutomationJobSnapshot>(`/sessions/${sessionId}/automation/configure`, {
    method: "POST",
    body: JSON.stringify(config)
  });
  automationDrafts.set(sessionId, cloneAutomationConfig(automation.config));
}

function rememberAutomationDraft(): void {
  if (!currentSession) return;
  automationDrafts.set(
    currentSession.session_id,
    cloneAutomationConfig(automationConfigFromControls())
  );
}

async function automationCommand(command: string): Promise<void> {
  if (!currentSession) return;
  await configureAutomation();
  renderSession(await api<SessionSnapshot>(
    `/sessions/${currentSession.session_id}/automation/${command}`,
    { method: "POST", body: "{}" }
  ));
}

async function automationControlCommand(command: string): Promise<void> {
  if (!currentSession) return;
  renderSession(await api<SessionSnapshot>(
    `/sessions/${currentSession.session_id}/automation/${command}`,
    { method: "POST", body: "{}" }
  ));
}

async function automationAutoPlay(): Promise<void> {
  if (!currentSession) return;
  await configureAutomation();
  renderSession(await api<SessionSnapshot>(
    `/sessions/${currentSession.session_id}/automation/auto-play`,
    { method: "POST", body: "{}" }
  ));
}

async function cancelAutomation(): Promise<void> {
  await automationControlCommand("cancel");
}

function clearSession(): void {
  currentSession = null;
  clearActiveSessionRefresh();
  clearPendingCommand();
  byId<HTMLDivElement>("status").textContent = "Not attached";
  byId<HTMLElement>("lifecycle").textContent = "not_attached";
  byId<HTMLElement>("trace").textContent = "-";
  renderFidelityChip("unknown");
  byId<HTMLElement>("reason").textContent = "-";
  byId<HTMLElement>("first-divergent").textContent = "-";
  byId<HTMLElement>("phase").textContent = "-";
  byId<HTMLElement>("state-freshness").textContent = "-";
  byId<HTMLElement>("session-alert").hidden = true;
  byId<HTMLPreElement>("diff").textContent = "";
  byId<HTMLButtonElement>("request").disabled = true;
  byId<HTMLButtonElement>("abandon").disabled = true;
  for (const id of [
    "automation-plan",
    "automation-run-one",
    "automation-auto-play",
    "automation-pause",
    "automation-resume",
    "automation-cancel"
  ]) {
    byId<HTMLButtonElement>(id).disabled = true;
  }
  byId<HTMLDivElement>("automation-potions").replaceChildren();
  byId<HTMLDivElement>("automation-summary").textContent = "No plan";
  renderCommandStatus();
  renderActions([]);
}

function selectedBridge(): BridgeStatus | null {
  const bridgeId = byId<HTMLSelectElement>("bridge").value;
  return bridgeStatuses.find((bridge) => bridge.id === bridgeId) || null;
}

function syncBridgeControls(): void {
  const hasBridge = bridgeStatuses.length > 0;
  const selected = selectedBridge();
  const canUseSelected = selected?.connected === true;
  byId<HTMLSelectElement>("bridge").disabled = !hasBridge;
  byId<HTMLButtonElement>("start").disabled = !canUseSelected;
  byId<HTMLButtonElement>("kill-selected").disabled = !selected;
  byId<HTMLButtonElement>("kill-all").disabled = !hasBridge;
}

async function killSelectedBridge(): Promise<void> {
  const bridgeId = byId<HTMLSelectElement>("bridge").value;
  if (!bridgeId) return;
  if (!window.confirm(`Kill bridge ${bridgeId}?`)) return;
  bridgeStatuses = [];
  syncBridgeControls();
  try {
    await api<unknown>(`/bridges/${bridgeId}/kill`, { method: "POST", body: "{}" });
    clearSession();
  } finally {
    await refreshBridges();
  }
}

async function killAllBridges(): Promise<void> {
  if (!window.confirm("Kill all bridge processes?")) return;
  bridgeStatuses = [];
  syncBridgeControls();
  try {
    await api<unknown>("/bridges/kill-all", { method: "POST", body: "{}" });
    clearSession();
  } finally {
    await refreshBridges();
  }
}

byId<HTMLButtonElement>("start").addEventListener("click", () => run(startRun));
byId<HTMLButtonElement>("load-session").addEventListener("click", () => run(loadSelectedSession));
byId<HTMLButtonElement>("request").addEventListener("click", () => run(requestState));
byId<HTMLButtonElement>("abandon").addEventListener("click", () => run(abandonRun));
byId<HTMLButtonElement>("kill-selected").addEventListener("click", () => run(killSelectedBridge));
byId<HTMLButtonElement>("kill-all").addEventListener("click", () => run(killAllBridges));
byId<HTMLButtonElement>("automation-plan").addEventListener("click", () => run(() => automationCommand("plan")));
byId<HTMLButtonElement>("automation-run-one").addEventListener("click", () => run(() => automationCommand("run-one")));
byId<HTMLButtonElement>("automation-auto-play").addEventListener("click", () => run(automationAutoPlay));
byId<HTMLButtonElement>("automation-pause").addEventListener("click", () => {
  run(() => automationControlCommand("pause"));
});
byId<HTMLButtonElement>("automation-resume").addEventListener("click", () => run(() => automationCommand("resume")));
byId<HTMLButtonElement>("automation-cancel").addEventListener("click", () => run(cancelAutomation));
byId<HTMLSelectElement>("bridge").addEventListener("change", syncBridgeControls);
byId<HTMLSelectElement>("automation-policy").addEventListener("change", rememberAutomationDraft);
for (const id of ["automation-depth", "automation-width", "automation-limit"]) {
  byId<HTMLInputElement>(id).addEventListener("input", rememberAutomationDraft);
}
byId<HTMLDivElement>("automation-potions").addEventListener("change", rememberAutomationDraft);

refreshBridges().catch(notifyError);
refreshSessions()
  .then(loadLatestSessionOnStartup)
  .catch(notifyError);
window.setInterval(() => refreshBridgesIfIdle().catch(console.error), BRIDGE_REFRESH_MS);
window.setInterval(() => refreshCurrentSessionIfIdle().catch(console.error), SESSION_REFRESH_MS);
window.setInterval(updateStateFreshness, 1000);
