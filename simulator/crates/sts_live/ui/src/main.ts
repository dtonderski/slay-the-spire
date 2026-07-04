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
  compact_diff?: string[];
  message?: string | null;
}

interface LegalAction {
  id: string;
  kind: string;
  label: string;
  enabled: boolean;
  disabled_reason?: string | null;
}

interface LiveState {
  phase: string;
  legal_actions: LegalAction[];
}

interface SessionSnapshot {
  session_id: string;
  bridge_id: string;
  lifecycle: SessionLifecycle;
  trace_path: string;
  run_config?: RunConfig | null;
  latest_state?: LiveState | null;
  fidelity: FidelityStatus;
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
const BRIDGE_REFRESH_MS = 1000;
const SESSION_REFRESH_MS = 2000;

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
  if (bridge.id.startsWith("local-process-")) {
    return `${bridge.client_id || "local bridge process"}${pid} (kill only)`;
  }
  if (bridge.connected) {
    return `${bridge.id}${pid}`;
  }
  return `${bridge.id}${pid} (not connected)`;
}

async function refreshSessions(): Promise<void> {
  const data = await api<SessionsResponse>("/sessions");
  const session = byId<HTMLSelectElement>("session");
  const selected = currentSession?.session_id || session.value;
  sessionStatuses = data.sessions;
  session.replaceChildren();
  for (const item of sessionStatuses) {
    const option = document.createElement("option");
    option.value = item.session_id;
    option.textContent = `${item.session_id} | ${item.lifecycle}`;
    session.appendChild(option);
  }
  if (selected) {
    session.value = selected;
  }
  syncBridgeControls();
}

function renderSession(session: SessionSnapshot): void {
  currentSession = session;
  byId<HTMLButtonElement>("request").disabled = false;
  byId<HTMLButtonElement>("abandon").disabled =
    !session.latest_state || session.latest_state.phase === "menu";
  byId<HTMLDivElement>("status").textContent = session.lifecycle;
  byId<HTMLElement>("trace").textContent = session.trace_path;
  byId<HTMLElement>("fidelity").textContent = session.fidelity.message
    ? `${session.fidelity.kind}: ${session.fidelity.message}`
    : session.fidelity.kind;
  byId<HTMLElement>("phase").textContent = session.latest_state?.phase || "-";
  byId<HTMLPreElement>("diff").textContent = [
    session.fidelity.message,
    ...(session.fidelity.compact_diff || [])
  ].filter(Boolean).join("\n");
  renderActions(session.latest_state?.legal_actions || []);
  syncBridgeControls();
  refreshSessions().catch(console.error);
}

function renderActions(actions: LegalAction[]): void {
  const container = byId<HTMLDivElement>("actions");
  container.replaceChildren();
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
      button.disabled = !action.enabled || !currentSession;
      button.title = action.disabled_reason || action.kind;
      button.addEventListener("click", () => {
        run(async () => {
          if (!currentSession) {
            throw new Error("No session selected");
          }
          const session = await api<SessionSnapshot>(
            `/sessions/${currentSession.session_id}/actions/${action.id}`,
            { method: "POST", body: "{}" }
          );
          renderSession(session);
        });
      });
      row.appendChild(button);
    }
    section.appendChild(row);
    container.appendChild(section);
  }
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

async function requestState(): Promise<void> {
  if (!currentSession) return;
  renderSession(await api<SessionSnapshot>(`/sessions/${currentSession.session_id}/request-state`, {
    method: "POST",
    body: "{}"
  }));
}

async function abandonRun(): Promise<void> {
  if (!currentSession) return;
  renderSession(await api<SessionSnapshot>(`/sessions/${currentSession.session_id}/abandon`, {
    method: "POST",
    body: "{}"
  }));
}

function clearSession(): void {
  currentSession = null;
  byId<HTMLDivElement>("status").textContent = "Not attached";
  byId<HTMLElement>("trace").textContent = "-";
  byId<HTMLElement>("fidelity").textContent = "unknown";
  byId<HTMLElement>("phase").textContent = "-";
  byId<HTMLPreElement>("diff").textContent = "";
  byId<HTMLButtonElement>("request").disabled = true;
  byId<HTMLButtonElement>("abandon").disabled = true;
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
byId<HTMLSelectElement>("bridge").addEventListener("change", syncBridgeControls);

refreshBridges().catch(notifyError);
refreshSessions().catch(notifyError);
window.setInterval(() => refreshBridges().catch(console.error), BRIDGE_REFRESH_MS);
window.setInterval(() => refreshCurrentSession().catch(console.error), SESSION_REFRESH_MS);
