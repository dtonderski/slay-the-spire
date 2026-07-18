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

interface HealthResponse {
  ok: boolean;
  backend?: string;
  service?: string;
}

interface SessionListItem {
  session_id: string;
  lifecycle: SessionLifecycle;
}

interface SessionsResponse {
  sessions: SessionListItem[];
}

interface ClearTracesResponse {
  deleted: number;
  sessions: SessionListItem[];
}

interface PermanentTraceResponse {
  path: string;
  run_id?: number | null;
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

interface SlayTheDataRunSummary {
  id: number;
  seed_played?: string | null;
  build_version?: string | null;
  ascension_level?: number | null;
  floor_reached?: number | null;
  victory: boolean;
  run_outcome: "win" | "loss" | "abandon";
  path_length?: number | null;
  card_choice_count?: number | null;
  event_choice_count?: number | null;
  shop_purchase_count?: number | null;
  potion_usage_count?: number | null;
  neow_bonus?: string | null;
  neow_cost?: string | null;
  guided_score: number;
  materialized: boolean;
}

interface SlayTheDataAdvisorStep {
  floor: number;
  ordinal: number;
  status: string;
  code: string;
  message: string;
  command?: string | null;
  action_id?: string | null;
  action_label?: string | null;
}

interface SlayTheDataSessionSnapshot {
  attached_run?: SlayTheDataRunSummary | null;
  advisor?: SlayTheDataAdvisorStep | null;
  next_step_index: number;
  blocked?: BlockedState | null;
  last_message?: string | null;
  auto_play_paused: boolean;
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
  slaythedata: SlayTheDataSessionSnapshot;
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

interface SlayTheDataSearchResponse {
  runs: SlayTheDataRunSummary[];
}

interface BrokenSlayTheDataRun {
  run_id: number;
  seed_played?: string | null;
  reason?: string | null;
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

type BackendConnectionState = "unknown" | "connected" | "disconnected";

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
let backendConnectionState: BackendConnectionState = "unknown";
let backendHealthInFlight = false;
let slayTheDataBusy = false;
let slayTheDataBusyMessage: string | null = null;
let lastSlayTheDataRuns: SlayTheDataRunSummary[] | null = null;
const BRIDGE_REFRESH_MS = 1000;
const SESSION_REFRESH_MS = 2000;
const BACKEND_HEALTH_MS = 1000;
const ACTIVE_SESSION_REFRESH_MS = 150;
const COMMAND_PENDING_TIMEOUT_MS = 3500;
const STS_SEED_ALPHABET = "0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ";
const U64_MODULUS = 1n << 64n;

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
  notify("Action failed", formatError(error), "notification");
}

function notifyInfo(message: string): void {
  notify("Done", message, "notification notification-info");
}

function notify(titleText: string, messageText: string, className: string): void {
  const container = byId<HTMLDivElement>("notifications");
  const notification = document.createElement("section");
  notification.className = className;

  const content = document.createElement("div");
  const title = document.createElement("div");
  title.className = "notification-title";
  title.textContent = titleText;
  const message = document.createElement("div");
  message.className = "notification-message";
  message.textContent = messageText;
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

function confirmAction(message: string): Promise<boolean> {
  const dialog = byId<HTMLDialogElement>("confirm-dialog");
  byId<HTMLParagraphElement>("confirm-dialog-message").textContent = message;
  dialog.returnValue = "cancel";
  dialog.showModal();
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => resolve(dialog.returnValue === "confirm"), { once: true });
  });
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

async function refreshBackendHealth(): Promise<void> {
  if (backendHealthInFlight) return;
  backendHealthInFlight = true;
  try {
    const health = await api<HealthResponse>("/health");
    backendConnectionState = health.ok ? "connected" : "disconnected";
    renderBackendStatus();
  } catch (error) {
    backendConnectionState = "disconnected";
    renderBackendStatus(formatError(error));
  } finally {
    backendHealthInFlight = false;
  }
}

function renderBackendStatus(detail?: string): void {
  const status = byId<HTMLDivElement>("backend-status");
  status.className = `backend-status backend-status-${backendConnectionState}`;
  if (backendConnectionState === "connected") {
    status.textContent = "Backend connected";
    status.title = "The UI can reach the live trace backend.";
  } else if (backendConnectionState === "disconnected") {
    status.textContent = "Backend disconnected";
    status.title = detail || "The UI cannot reach the live trace backend.";
  } else {
    status.textContent = "Backend unknown";
    status.title = "Waiting for backend health check.";
  }
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
  syncRunControlsFromSession(session);
  byId<HTMLButtonElement>("request").disabled = false;
  byId<HTMLButtonElement>("abandon").disabled =
    !session.latest_state || session.latest_state.phase === "menu";
  byId<HTMLButtonElement>("clear-traces").disabled = false;
  byId<HTMLButtonElement>("add-to-permanent-corpus").disabled = false;
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
  renderSlayTheData(session);
  if (lastSlayTheDataRuns) {
    renderSlayTheDataResults(lastSlayTheDataRuns);
  }
  syncBridgeControls();
  scheduleActiveSessionRefresh(session);
  refreshSessions().catch(console.error);
}

function syncRunControlsFromSession(session: SessionSnapshot): void {
  const config = session.run_config;
  if (!config) return;
  byId<HTMLSelectElement>("character").value = config.character;
  byId<HTMLInputElement>("ascension").value = String(config.ascension);
  byId<HTMLInputElement>("slaythedata-ascension").value = String(config.ascension);
  byId<HTMLInputElement>("seed").value = seedInputText(config.seed);
}

function seedInputText(seed: RunSeed): string {
  if ("external" in seed) return seed.external;
  if ("numeric" in seed) return stsSeedLongToString(String(seed.numeric));
  return "";
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
    renderSlayTheData(currentSession);
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
    "waiting_for_live_state",
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
            void refreshFidelityForRenderedState(
              session.session_id,
              session.latest_state?.sequence ?? null
            ).catch(console.error);
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

function renderSlayTheData(session: SessionSnapshot): void {
  const state = session.slaythedata || defaultSlayTheData();
  const canGuide = Boolean(session.latest_state && session.lifecycle !== "ended" && state.attached_run);
  const pending = isCommandPending() || slayTheDataBusy;
  const hasAdvisor = Boolean(state.advisor);
  const canRetryGuidance = state.blocked?.reason_code === "slaythedata_no_live_action"
    || state.blocked?.reason_code === "pending_card_reward"
    || Boolean(state.blocked?.reason_code.startsWith("guided_"));
  byId<HTMLButtonElement>("slaythedata-send-next").disabled =
    !canGuide || pending || (Boolean(state.blocked) && !canRetryGuidance) || !hasAdvisor;
  byId<HTMLButtonElement>("slaythedata-auto-play").disabled =
    !canGuide || pending || (Boolean(state.blocked) && !canRetryGuidance) || !hasAdvisor;
  byId<HTMLButtonElement>("slaythedata-pause").disabled =
    !canGuide || state.auto_play_paused;
  byId<HTMLButtonElement>("slaythedata-skip-shop").disabled =
    !canGuide || state.blocked?.reason_code !== "shop_purchase_unavailable";
  byId<HTMLButtonElement>("slaythedata-search").disabled = pending;
  byId<HTMLButtonElement>("slaythedata-search-current").disabled =
    pending || !currentSessionSeedPlayed();

  const advisor = byId<HTMLDivElement>("slaythedata-advisor");
  advisor.replaceChildren();
  if (state.attached_run) {
    const run = document.createElement("div");
    const build = state.attached_run.build_version
      ? ` | build ${state.attached_run.build_version}`
      : "";
    run.textContent = `Run ${state.attached_run.id} | seed ${state.attached_run.seed_played || "-"} | floor ${state.attached_run.floor_reached ?? "-"} | ${formatRunOutcome(state.attached_run.run_outcome)}${build}`;
    advisor.appendChild(run);
  }
  if (state.blocked) {
    const blocked = document.createElement("div");
    blocked.className = "automation-blocked";
    blocked.textContent = `${state.blocked.reason_code}: ${state.blocked.message}`;
    advisor.appendChild(blocked);
  }
  if (state.last_message) {
    const status = document.createElement("div");
    status.className = "slaythedata-status";
    status.textContent = state.last_message;
    advisor.appendChild(status);
  }
  if (state.advisor) {
    const code = document.createElement("div");
    code.className = "advisor-code";
    code.textContent = `${state.advisor.status} ${state.advisor.code}`;
    const message = document.createElement("div");
    message.textContent = state.advisor.action_label
      ? `${state.advisor.message} -> ${state.advisor.action_label}`
      : state.advisor.message;
    advisor.append(code, message);
  } else if (!state.attached_run) {
    advisor.textContent = "No advisor";
  } else if (!state.blocked && !state.last_message) {
    advisor.append("No remaining non-combat guidance");
  }
}

function defaultSlayTheData(): SlayTheDataSessionSnapshot {
  return {
    attached_run: null,
    advisor: null,
    next_step_index: 0,
    blocked: null,
    last_message: null,
    auto_play_paused: false
  };
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

function slayTheDataRunConfig(runSummary: SlayTheDataRunSummary): RunConfig {
  const seed = slayTheDataSeedText(runSummary);
  byId<HTMLInputElement>("seed").value = seed;
  byId<HTMLInputElement>("ascension").value = String(runSummary.ascension_level ?? 0);
  return {
    character: "ironclad",
    ascension: runSummary.ascension_level ?? 0,
    seed: { external: seed }
  };
}

function slayTheDataSeedText(runSummary: SlayTheDataRunSummary): string {
  const seed = runSummary.seed_played?.trim();
  if (!seed) {
    throw new Error(`SlayTheData run ${runSummary.id} has no seed`);
  }
  return /^-?\d+$/.test(seed) ? stsSeedLongToString(seed) : seed;
}

function stsSeedLongToString(seed: string): string {
  let value = BigInt(seed);
  if (value < 0n) {
    value += U64_MODULUS;
  }
  if (value === 0n) return "";
  const radix = BigInt(STS_SEED_ALPHABET.length);
  let encoded = "";
  while (value !== 0n) {
    const digit = Number(value % radix);
    value /= radix;
    encoded = STS_SEED_ALPHABET[digit] + encoded;
  }
  return encoded;
}

async function startSlayTheDataRun(runSummary: SlayTheDataRunSummary): Promise<SessionSnapshot> {
  const bridge = selectedBridge();
  if (!bridge?.connected) {
    throw new Error("Select a connected bridge before starting a SlayTheData run");
  }
  if (currentSession?.latest_state?.phase === "game_over") {
    renderSession(await api<SessionSnapshot>(
      `/sessions/${currentSession.session_id}/abandon`,
      { method: "POST", body: "{}" }
    ));
  }
  return api<SessionSnapshot>("/sessions/start", {
    method: "POST",
    body: JSON.stringify({
      bridge_id: bridge.id,
      config: slayTheDataRunConfig(runSummary)
    })
  });
}

async function startAndAttachSlayTheDataRun(runSummary: SlayTheDataRunSummary): Promise<void> {
  const session = await startSlayTheDataRun(runSummary);
  renderSession(session);
  await attachSlayTheDataRunToSession(session.session_id, runSummary);
}

async function attachSlayTheDataRun(runSummary: SlayTheDataRunSummary): Promise<void> {
  if (!currentSession) {
    throw new Error("Open a current session before attaching without starting");
  }
  await attachSlayTheDataRunToSession(currentSession.session_id, runSummary);
}

async function attachSlayTheDataRunToSession(
  sessionId: string,
  runSummary: SlayTheDataRunSummary
): Promise<void> {
  const session = await api<SessionSnapshot>(
    `/sessions/${sessionId}/slaythedata/attach`,
    { method: "POST", body: JSON.stringify({ run_id: runSummary.id }) }
  );
  const attachedRun = session.slaythedata?.attached_run;
  if (attachedRun?.id === runSummary.id) {
    Object.assign(runSummary, attachedRun);
    if (lastSlayTheDataRuns) renderSlayTheDataResults(lastSlayTheDataRuns);
  }
  renderSession(session);
}

async function downloadSlayTheDataJson(runSummary: SlayTheDataRunSummary): Promise<void> {
  const payload = await api<unknown>(`/slaythedata/runs/${runSummary.id}/json`);
  const raw = payload && typeof payload === "object"
    ? payload as Record<string, unknown>
    : null;
  const event = raw?.event && typeof raw.event === "object"
    ? raw.event as Record<string, unknown>
    : raw;
  const buildVersion = event?.build_version;
  runSummary.materialized = true;
  runSummary.build_version = typeof buildVersion === "string" ? buildVersion : null;
  if (lastSlayTheDataRuns) renderSlayTheDataResults(lastSlayTheDataRuns);
  const jsonText = JSON.stringify(payload, null, 2);
  const blob = new Blob([jsonText, "\n"], { type: "application/json" });
  const link = document.createElement("a");
  const url = URL.createObjectURL(blob);
  link.href = url;
  link.download = `slaythedata-run-${runSummary.id}.json`;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

async function markSlayTheDataRunBroken(runSummary: SlayTheDataRunSummary): Promise<void> {
  const label = runSummary.seed_played ? `seed ${runSummary.seed_played}` : `run ${runSummary.id}`;
  if (!await confirmAction(`Mark ${label} as broken and hide it from future SlayTheData searches?`)) {
    return;
  }
  const broken = await api<BrokenSlayTheDataRun>(
    `/slaythedata/runs/${runSummary.id}/mark-broken`,
    {
      method: "POST",
      body: JSON.stringify({ reason: "marked broken from UI" })
    }
  );
  if (lastSlayTheDataRuns) {
    lastSlayTheDataRuns = lastSlayTheDataRuns.filter((run) => {
      if (run.id === broken.run_id) return false;
      return !broken.seed_played || run.seed_played !== broken.seed_played;
    });
    renderSlayTheDataResults(lastSlayTheDataRuns);
  }
  notifyInfo(`Marked ${label} broken.`);
}

async function loadSelectedSession(): Promise<void> {
  const sessionId = byId<HTMLSelectElement>("session").value;
  if (!sessionId) return;
  renderSession(await api<SessionSnapshot>(`/sessions/${sessionId}`));
}

async function clearOtherTraces(): Promise<void> {
  if (!currentSession) return;
  const confirmed = window.confirm(
    `Delete all saved traces except ${currentSession.session_id}? This cannot be undone.`
  );
  if (!confirmed) return;
  const response = await api<ClearTracesResponse>(
    `/sessions/${currentSession.session_id}/clear-other-traces`,
    { method: "POST", body: "{}" }
  );
  sessionStatuses = response.sessions;
  await refreshSessions();
  notifyInfo(`Deleted ${response.deleted} trace${response.deleted === 1 ? "" : "s"}.`);
}

async function addCurrentTraceToPermanentCorpus(): Promise<void> {
  if (!currentSession) return;
  const response = await api<PermanentTraceResponse>(
    `/sessions/${currentSession.session_id}/add-to-permanent-corpus`,
    { method: "POST", body: "{}" }
  );
  if (
    response.run_id != null
    && byId<HTMLSelectElement>("slaythedata-corpus-runs").value === "exclude"
    && lastSlayTheDataRuns
  ) {
    lastSlayTheDataRuns = lastSlayTheDataRuns.filter((run) => run.id !== response.run_id);
    renderSlayTheDataResults(lastSlayTheDataRuns);
  }
  const run = response.run_id == null ? "" : `; SlayTheData run ${response.run_id} marked added`;
  notifyInfo(`Added trace to permanent corpus: ${response.path}${run}`);
}

async function refreshCurrentSession(): Promise<void> {
  if (!currentSession) return;
  const sessionId = currentSession.session_id;
  const session = await api<SessionSnapshot>(`/sessions/${sessionId}`);
  if (currentSession?.session_id === sessionId) {
    renderSession(session);
  }
}

async function refreshFidelityForRenderedState(
  sessionId: string,
  sourceSequence: number | null
): Promise<void> {
  const session = await api<SessionSnapshot["fidelity"]>(`/sessions/${sessionId}/fidelity`);
  if (
    currentSession?.session_id === sessionId &&
    (sourceSequence === null || (currentSession.latest_state?.sequence ?? null) === sourceSequence)
  ) {
    currentSession.fidelity = session;
    currentSession.lifecycle = session.kind === "ok" ? "fidelity_ok" : session.kind === "lost" ? "fidelity_lost" : currentSession.lifecycle;
    renderSession(currentSession);
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
  if (!await confirmAction(`Abandon run for ${currentSession.session_id}?`)) return;
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

function slayTheDataFilters(seedPlayed?: string): Record<string, unknown> {
  const outcome = byId<HTMLSelectElement>("slaythedata-outcome").value;
  const neowBonus = byId<HTMLSelectElement>("slaythedata-neow-bonus").value;
  const corpusRuns = byId<HTMLSelectElement>("slaythedata-corpus-runs").value;
  const runId = byId<HTMLInputElement>("slaythedata-run-id").value.trim();
  return {
    character: "IRONCLAD",
    run_id: runId === "" ? null : Number(runId),
    ascension: Number(byId<HTMLInputElement>("slaythedata-ascension").value),
    min_floor_reached: Number(byId<HTMLInputElement>("slaythedata-min-floor").value),
    run_outcome: outcome === "any" ? null : outcome,
    neow_bonus: neowBonus === "any" ? null : neowBonus,
    include_corpus: corpusRuns === "include",
    seed_played: seedPlayed ?? null,
    limit: Number(byId<HTMLInputElement>("slaythedata-limit").value),
    require_supported: true
  };
}

async function searchSlayTheData(): Promise<void> {
  const response = await api<SlayTheDataSearchResponse>("/slaythedata/search", {
    method: "POST",
    body: JSON.stringify(slayTheDataFilters())
  });
  lastSlayTheDataRuns = response.runs;
  renderSlayTheDataResults(response.runs);
}

async function searchCurrentSlayTheDataSeed(): Promise<void> {
  const seedPlayed = currentSessionSeedPlayed();
  if (!seedPlayed) {
    throw new Error("Current session has no searchable seed");
  }
  if (currentSession?.run_config?.ascension != null) {
    byId<HTMLInputElement>("slaythedata-ascension").value = String(currentSession.run_config.ascension);
  }
  const response = await api<SlayTheDataSearchResponse>("/slaythedata/search", {
    method: "POST",
    body: JSON.stringify(slayTheDataFilters(seedPlayed))
  });
  lastSlayTheDataRuns = response.runs;
  renderSlayTheDataResults(response.runs);
}

function currentSessionSeedPlayed(): string | null {
  const seed = currentSession?.run_config?.seed;
  if (!seed) return null;
  if ("external" in seed) {
    return stsSeedStringToLongString(seed.external);
  }
  if ("numeric" in seed && Number.isSafeInteger(seed.numeric)) {
    return String(seed.numeric);
  }
  return null;
}

function stsSeedStringToLongString(seed: string): string {
  let value = 0n;
  const radix = BigInt(STS_SEED_ALPHABET.length);
  for (const raw of seed.toUpperCase().replace(/O/g, "0")) {
    const digit = STS_SEED_ALPHABET.indexOf(raw);
    if (digit < 0) {
      throw new Error(`Current seed contains invalid STS seed character: ${raw}`);
    }
    value = (value * radix + BigInt(digit)) % U64_MODULUS;
  }
  const signedMax = (1n << 63n) - 1n;
  if (value > signedMax) {
    value -= U64_MODULUS;
  }
  return value.toString();
}

function formatRunOutcome(outcome: SlayTheDataRunSummary["run_outcome"]): string {
  switch (outcome) {
    case "win":
      return "win";
    case "loss":
      return "loss";
    case "abandon":
      return "abandon";
  }
}

function renderSlayTheDataResults(runs: SlayTheDataRunSummary[]): void {
  const container = byId<HTMLDivElement>("slaythedata-results");
  const bridge = selectedBridge();
  const renderKey = JSON.stringify({
    runs,
    busy: slayTheDataBusy,
    busyMessage: slayTheDataBusyMessage,
    sessionId: currentSession?.session_id ?? null,
    bridgeId: bridge?.id ?? null,
    bridgeConnected: bridge?.connected ?? false
  });
  if (container.dataset.renderKey === renderKey) return;
  container.dataset.renderKey = renderKey;
  container.replaceChildren();
  if (slayTheDataBusyMessage) {
    const status = document.createElement("div");
    status.className = "slaythedata-status";
    status.textContent = slayTheDataBusyMessage;
    container.appendChild(status);
  }
  if (runs.length === 0) {
    const empty = document.createElement("div");
    empty.textContent = slayTheDataBusyMessage ? "Waiting for results..." : "No matching runs";
    container.appendChild(empty);
    return;
  }
  for (const runSummary of runs) {
    const row = document.createElement("div");
    row.className = "slaythedata-run";
    const label = document.createElement("div");
    label.className = "slaythedata-run-summary";
    const build = runSummary.materialized && runSummary.build_version
      ? ` | build ${runSummary.build_version}`
      : "";
    label.textContent = `#${runSummary.id} ${runSummary.seed_played || "-"} | A${runSummary.ascension_level ?? "-"} | floor ${runSummary.floor_reached ?? "-"} | ${formatRunOutcome(runSummary.run_outcome)}${build} | ${runSummary.materialized ? "ready" : "not materialized"}`;
    const actions = document.createElement("div");
    actions.className = "slaythedata-run-actions";
    const startButton = document.createElement("button");
    startButton.type = "button";
    startButton.textContent = "Start + attach";
    const canStartRun = Boolean(runSummary.seed_played && selectedBridge()?.connected);
    startButton.disabled = slayTheDataBusy || !canStartRun;
    if (canStartRun) {
      startButton.title = runSummary.materialized
        ? "Start this SlayTheData seed and attach the run"
        : "Start this SlayTheData seed, materialize it, and attach the run";
    } else {
      startButton.title = runSummary.seed_played
        ? "Select a connected bridge to start this SlayTheData seed"
        : "This SlayTheData run has no seed";
    }
    startButton.addEventListener("click", () => {
      lastSlayTheDataRuns = [runSummary];
      run(() => runSlayTheDataTask(() => startAndAttachSlayTheDataRun(runSummary)));
    });

    const attachButton = document.createElement("button");
    attachButton.type = "button";
    attachButton.textContent = "Attach";
    attachButton.disabled = slayTheDataBusy || !currentSession;
    attachButton.title = currentSession
      ? "Attach this SlayTheData run to the current session"
      : "Open or start a current session before attaching";
    attachButton.addEventListener("click", () => {
      run(() => runSlayTheDataTask(() => attachSlayTheDataRun(runSummary)));
    });

    const jsonButton = document.createElement("button");
    jsonButton.type = "button";
    jsonButton.textContent = "JSON";
    jsonButton.disabled = slayTheDataBusy;
    jsonButton.title = runSummary.materialized
      ? "Download the raw SlayTheData run JSON"
      : "Materialize and download the raw SlayTheData run JSON";
    jsonButton.addEventListener("click", () => {
      run(() => runSlayTheDataTask(() => downloadSlayTheDataJson(runSummary)));
    });

    const brokenButton = document.createElement("button");
    brokenButton.type = "button";
    brokenButton.textContent = "Broken";
    brokenButton.disabled = slayTheDataBusy;
    brokenButton.title = "Hide this SlayTheData seed from future search results";
    brokenButton.addEventListener("click", () => {
      run(() => runSlayTheDataTask(
        () => markSlayTheDataRunBroken(runSummary),
        "Marking SlayTheData seed broken..."
      ));
    });

    actions.append(startButton, attachButton, jsonButton, brokenButton);
    row.append(label, actions);
    container.appendChild(row);
  }
}

function renderSlayTheDataBusyState(): void {
  if (!slayTheDataBusyMessage) return;
  const container = byId<HTMLDivElement>("slaythedata-results");
  delete container.dataset.renderKey;
  container.replaceChildren();
  const status = document.createElement("div");
  status.className = "slaythedata-status";
  status.textContent = slayTheDataBusyMessage;
  container.appendChild(status);
}

async function slayTheDataCommand(command: "send-next" | "auto-play" | "pause" | "skip-shop"): Promise<void> {
  if (!currentSession) return;
  renderSession(await api<SessionSnapshot>(
    `/sessions/${currentSession.session_id}/slaythedata/${command}`,
    { method: "POST", body: "{}" }
  ));
}

async function runSlayTheDataTask(
  task: () => Promise<void>,
  busyMessage = "SlayTheData action pending..."
): Promise<void> {
  if (slayTheDataBusy) return;
  slayTheDataBusy = true;
  slayTheDataBusyMessage = busyMessage;
  if (currentSession) {
    renderSlayTheData(currentSession);
  }
  if (lastSlayTheDataRuns) {
    renderSlayTheDataResults(lastSlayTheDataRuns);
  } else {
    renderSlayTheDataBusyState();
  }
  try {
    await task();
  } finally {
    slayTheDataBusy = false;
    slayTheDataBusyMessage = null;
    if (currentSession) {
      renderSlayTheData(currentSession);
    }
    if (lastSlayTheDataRuns) {
      renderSlayTheDataResults(lastSlayTheDataRuns);
    }
  }
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
  byId<HTMLButtonElement>("clear-traces").disabled = true;
  byId<HTMLButtonElement>("add-to-permanent-corpus").disabled = true;
  for (const id of [
    "automation-plan",
    "automation-run-one",
    "automation-auto-play",
    "automation-pause",
    "automation-resume",
    "automation-cancel",
    "slaythedata-search-current",
    "slaythedata-send-next",
    "slaythedata-auto-play",
    "slaythedata-pause",
    "slaythedata-skip-shop"
  ]) {
    byId<HTMLButtonElement>(id).disabled = true;
  }
  byId<HTMLDivElement>("automation-potions").replaceChildren();
  byId<HTMLDivElement>("automation-summary").textContent = "No plan";
  lastSlayTheDataRuns = null;
  byId<HTMLDivElement>("slaythedata-results").textContent = "No run selected";
  byId<HTMLDivElement>("slaythedata-advisor").textContent = "No advisor";
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
  if (lastSlayTheDataRuns) {
    renderSlayTheDataResults(lastSlayTheDataRuns);
  }
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
byId<HTMLButtonElement>("clear-traces").addEventListener("click", () => run(clearOtherTraces));
byId<HTMLButtonElement>("add-to-permanent-corpus").addEventListener("click", () => {
  run(addCurrentTraceToPermanentCorpus);
});
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
byId<HTMLButtonElement>("slaythedata-search").addEventListener("click", () => run(() => runSlayTheDataTask(searchSlayTheData, "Searching SlayTheData...")));
byId<HTMLButtonElement>("slaythedata-search-current").addEventListener("click", () => run(() => runSlayTheDataTask(searchCurrentSlayTheDataSeed, "Searching current seed...")));
byId<HTMLButtonElement>("slaythedata-send-next").addEventListener("click", () => run(() => runSlayTheDataTask(() => slayTheDataCommand("send-next"))));
byId<HTMLButtonElement>("slaythedata-auto-play").addEventListener("click", () => run(() => runSlayTheDataTask(() => slayTheDataCommand("auto-play"))));
byId<HTMLButtonElement>("slaythedata-pause").addEventListener("click", () => run(() => slayTheDataCommand("pause")));
byId<HTMLButtonElement>("slaythedata-skip-shop").addEventListener("click", () => run(() => slayTheDataCommand("skip-shop")));
byId<HTMLSelectElement>("bridge").addEventListener("change", syncBridgeControls);
byId<HTMLSelectElement>("automation-policy").addEventListener("change", rememberAutomationDraft);
for (const id of ["automation-depth", "automation-width", "automation-limit"]) {
  byId<HTMLInputElement>(id).addEventListener("input", rememberAutomationDraft);
}
byId<HTMLDivElement>("automation-potions").addEventListener("change", rememberAutomationDraft);

renderBackendStatus();
refreshBackendHealth().catch(console.error);
refreshBridges().catch(notifyError);
refreshSessions()
  .then(loadLatestSessionOnStartup)
  .catch(notifyError);
window.setInterval(() => refreshBackendHealth().catch(console.error), BACKEND_HEALTH_MS);
window.setInterval(() => refreshBridgesIfIdle().catch(console.error), BRIDGE_REFRESH_MS);
window.setInterval(() => refreshCurrentSessionIfIdle().catch(console.error), SESSION_REFRESH_MS);
window.setInterval(updateStateFreshness, 1000);
