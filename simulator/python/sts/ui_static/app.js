(function () {
  "use strict";

  const app = {
    sessionId: null,
    state: null,
    snapshot: null,
    actions: [],
    lastActions: [],
    inFlight: false,
    lifecycle: { kind: "Ready" },
    lastError: null,
    search: null,
    bridge: null,
    parity: null,
    traces: [],
    selectedTraceId: "",
    traceDetail: null,
    traceRecords: [],
    traceError: null,
    traceLoading: false,
    activeDebugTab: "state",
    mode: null,
    stateKind: null,
    phase: null,
    currentDecision: null,
    unsupportedReason: null,
  };

  const el = {};

  document.addEventListener("DOMContentLoaded", () => {
    bindElements();
    bindEvents();
    render();
  });

  function bindElements() {
    for (const id of [
      "sessionMeta",
      "sessionModeSelect",
      "newSessionButton",
      "reloadButton",
      "stateSummary",
      "lifecycleBadge",
      "playerPanel",
      "monsterPanel",
      "energyStat",
      "drawStat",
      "discardStat",
      "exhaustStat",
      "handCount",
      "handPanel",
      "actionCount",
      "actionError",
      "pendingMessage",
      "actionsPanel",
      "maxDepthInput",
      "searchButton",
      "applyBestButton",
      "searchStatus",
      "searchResult",
      "refreshBridgeButton",
      "requestBridgeStateButton",
      "bridgePanel",
      "refreshTracesButton",
      "loadTraceButton",
      "traceSelect",
      "traceOffsetInput",
      "traceLimitInput",
      "traceStatus",
      "traceError",
      "traceMetaPanel",
      "traceRecordsPanel",
      "debugSessionId",
      "debugMode",
      "debugStateKind",
      "debugStateId",
      "debugPhase",
      "debugDecision",
      "debugUnsupported",
      "debugHash",
      "debugJson",
    ]) {
      el[id] = document.getElementById(id);
    }
    el.debugTabs = Array.from(document.querySelectorAll("[data-debug-tab]"));
  }

  function bindEvents() {
    el.newSessionButton.addEventListener("click", startSession);
    el.reloadButton.addEventListener("click", reloadSession);
    el.searchButton.addEventListener("click", runSearch);
    el.applyBestButton.addEventListener("click", applyBestAction);
    el.refreshBridgeButton.addEventListener("click", refreshBridge);
    el.requestBridgeStateButton.addEventListener("click", requestBridgeState);
    el.refreshTracesButton.addEventListener("click", refreshTraces);
    el.loadTraceButton.addEventListener("click", loadSelectedTrace);
    el.traceSelect.addEventListener("change", () => {
      app.selectedTraceId = el.traceSelect.value;
      app.traceDetail = selectedTraceMetadata();
      app.traceRecords = [];
      app.traceError = null;
      renderTrace();
    });
    el.debugTabs.forEach((button) => {
      button.addEventListener("click", () => {
        app.activeDebugTab = button.dataset.debugTab;
        renderDebug();
      });
    });
  }

  async function startSession() {
    const mode = el.sessionModeSelect.value || "combat_fixture";
    await singleFlight("Starting fixture", async () => {
      const session = await requestJson("/api/sessions", {
        method: "POST",
        body: { mode },
      });
      adoptSession(session);
      app.snapshot = null;
      app.search = null;
      await loadSnapshotQuietly();
      await refreshBridgeQuietly();
      await refreshParityQuietly();
    });
  }

  async function reloadSession() {
    if (!app.sessionId) return;
    await singleFlight("Reloading session", async () => {
      const session = await requestJson(`/api/sessions/${encodeURIComponent(app.sessionId)}`);
      adoptSession(session);
      await loadSnapshotQuietly();
      await refreshBridgeQuietly();
      await refreshParityQuietly();
    });
  }

  async function runSearch() {
    if (!app.sessionId || !isCombatSession()) return;
    const maxDepth = Number.parseInt(el.maxDepthInput.value, 10);
    await singleFlight("Searching", async () => {
      app.search = null;
      renderSearch();
      const recommendation = await requestJson(
        `/api/sessions/${encodeURIComponent(app.sessionId)}/search`,
        {
          method: "POST",
          body: { max_depth: Number.isFinite(maxDepth) ? maxDepth : undefined },
        },
      );
      app.search = normalizeSearch(recommendation);
    });
  }

  async function applyBestAction() {
    const best = app.search && app.search.bestAction;
    if (!best) return;
    await submitAction(best);
  }

  async function submitAction(action) {
    if (!app.sessionId || app.inFlight) {
      flashPending();
      return;
    }

    const sourceStateId = action.source_state_id || action.sourceStateId || currentStateId();
    const actionId = action.action_id || action.id;
    if (!actionId) {
      showError("Cannot submit this action because it has no action_id.");
      return;
    }

    await singleFlight(`Submitting ${actionLabel(action)}`, async () => {
      const result = await requestJson(`/api/sessions/${encodeURIComponent(app.sessionId)}/step`, {
        method: "POST",
        body: { action_id: actionId, source_state_id: sourceStateId },
      });
      adoptSession(result.session || result.state || result);
      app.search = null;
      await loadSnapshotQuietly();
      await refreshParityQuietly();
      const lifecycle = firstDefined(result.command_lifecycle, result.commandLifecycle, null);
      if (lifecycle && (lifecycle.status === "stale" || lifecycle.status === "rejected")) {
        throw new Error(firstDefined(lifecycle.error, result.last_error, "Action was rejected."));
      }
    }, {
      sourceStateId,
      action,
    });
  }

  async function singleFlight(label, work, pending) {
    if (app.inFlight) {
      flashPending();
      return;
    }
    app.inFlight = true;
    app.lastError = null;
    app.lifecycle = {
      kind: "Submitting",
      label,
      sourceStateId: pending && pending.sourceStateId,
      action: pending && pending.action,
    };
    render();

    try {
      await work();
      app.lifecycle = { kind: "Applied", stateId: currentStateId() };
    } catch (error) {
      app.lastError = readableError(error);
      app.lifecycle = {
        kind: isStaleError(error) ? "Stale" : "Rejected",
        error: app.lastError,
        stateId: currentStateId(),
      };
    } finally {
      app.inFlight = false;
      render();
    }
  }

  async function requestJson(url, options) {
    const init = {
      method: options && options.method ? options.method : "GET",
      headers: { Accept: "application/json" },
    };
    if (options && Object.prototype.hasOwnProperty.call(options, "body")) {
      init.headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(options.body || {});
    }

    const response = await fetch(url, init);
    const text = await response.text();
    let data = null;
    if (text) {
      try {
        data = JSON.parse(text);
      } catch (error) {
        data = { error: text };
      }
    }

    if (!response.ok) {
      const message = errorFromPayload(data) || `${response.status} ${response.statusText}`;
      const error = new Error(message);
      error.status = response.status;
      error.payload = data;
      throw error;
    }
    return data || {};
  }

  async function loadSnapshotQuietly() {
    if (!app.sessionId) return;
    try {
      app.snapshot = await requestJson(`/api/sessions/${encodeURIComponent(app.sessionId)}/snapshot`);
    } catch (error) {
      app.snapshot = { error: readableError(error) };
    }
  }

  async function refreshBridge() {
    await singleFlight("Refreshing bridge", async () => {
      await refreshBridgeQuietly();
      await refreshParityQuietly();
    });
  }

  async function refreshBridgeQuietly() {
    try {
      app.bridge = await requestJson("/api/bridge");
    } catch (error) {
      app.bridge = { error: readableError(error), connected: false, stale: true };
    }
  }

  async function refreshParityQuietly() {
    if (!app.sessionId) return;
    try {
      const result = await requestJson(`/api/sessions/${encodeURIComponent(app.sessionId)}/parity`);
      app.parity = result.parity || result;
    } catch (error) {
      app.parity = { status: "unknown", reason: readableError(error), diffs: [] };
    }
  }

  async function requestBridgeState() {
    await singleFlight("Requesting bridge state", async () => {
      const result = await requestJson("/api/bridge/command", {
        method: "POST",
        body: { command: "state" },
      });
      app.bridge = result.bridge_status || result.bridgeStatus || app.bridge;
      await refreshParityQuietly();
    });
  }

  async function submitBridgeAction(action) {
    if (app.inFlight) {
      flashPending();
      return;
    }
    const descriptor = action && action.descriptor;
    if (!descriptor) {
      showError("Cannot submit this bridge action because it has no descriptor.");
      return;
    }

    await singleFlight(`Sending ${bridgeActionLabel(action)}`, async () => {
      const result = await requestJson("/api/bridge/descriptor", {
        method: "POST",
        body: { descriptor },
      });
      app.bridge = result.bridge_status || result.bridgeStatus || app.bridge;
      await refreshParityQuietly();
    });
  }

  async function refreshTraces() {
    app.traceLoading = true;
    app.traceError = null;
    renderTrace();
    try {
      const result = await requestJson("/api/traces");
      app.traces = arrayOf(result.traces);
      if (!app.traces.some((trace) => String(trace.id) === app.selectedTraceId)) {
        app.selectedTraceId = app.traces.length ? String(app.traces[0].id) : "";
      }
      app.traceDetail = selectedTraceMetadata();
      app.traceRecords = [];
    } catch (error) {
      app.traces = [];
      app.selectedTraceId = "";
      app.traceDetail = null;
      app.traceRecords = [];
      app.traceError = readableError(error);
    } finally {
      app.traceLoading = false;
      renderTrace();
    }
  }

  async function loadSelectedTrace() {
    if (!app.selectedTraceId) return;
    app.traceLoading = true;
    app.traceError = null;
    renderTrace();
    try {
      const offset = boundedInteger(el.traceOffsetInput.value, 0, 0, Number.MAX_SAFE_INTEGER);
      const limit = boundedInteger(el.traceLimitInput.value, 200, 1, 1000);
      const query = new URLSearchParams({ offset: String(offset), limit: String(limit) });
      const result = await requestJson(`/api/traces/${encodeURIComponent(app.selectedTraceId)}?${query}`);
      app.traceDetail = result.trace || selectedTraceMetadata();
      app.traceRecords = arrayOf(result.records);
    } catch (error) {
      app.traceRecords = [];
      app.traceError = readableError(error);
    } finally {
      app.traceLoading = false;
      renderTrace();
    }
  }

  function adoptSession(payload) {
    const state = payload && (payload.state || payload.ui_state || payload);
    app.sessionId = firstDefined(payload && payload.id, payload && payload.session_id, state && state.session_id, app.sessionId);
    app.state = state || null;
    app.mode = firstDefined(payload && payload.mode, state && state.mode, app.mode);
    app.stateKind = firstDefined(payload && payload.state_kind, payload && payload.stateKind, state && state.state_kind, state && state.stateKind, app.stateKind);
    app.phase = firstDefined(payload && payload.phase, state && state.phase, app.phase);
    app.currentDecision = firstDefined(payload && payload.current_decision, payload && payload.currentDecision, state && state.current_decision, state && state.currentDecision, app.currentDecision);
    app.unsupportedReason = firstDefined(payload && payload.unsupported_reason, payload && payload.unsupportedReason, state && state.unsupported_reason, state && state.unsupportedReason, null);
    app.parity = firstDefined(payload && payload.parity, state && state.parity, app.parity);
    app.lastError = firstDefined(payload && payload.last_error, state && state.last_error, app.lastError);
    app.lifecycle = lifecycleFromPayload(firstDefined(
      payload && payload.command_lifecycle,
      payload && payload.commandLifecycle,
      state && state.command_lifecycle,
      state && state.commandLifecycle,
      null,
    ));

    const actions = firstDefined(
      payload && payload.actions,
      state && state.actions,
      state && state.available_actions,
      state && state.visible_controls,
      state && state.exact_legal_actions,
      [],
    );
    if (Array.isArray(actions)) {
      app.actions = actions;
      app.lastActions = actions;
    } else {
      app.actions = [];
    }
    if (!isCombatSession()) {
      app.search = null;
    }
  }

  function render() {
    renderChrome();
    renderBoard();
    renderHand();
    renderActions();
    renderSearch();
    renderBridge();
    renderTrace();
    renderDebug();
  }

  function renderChrome() {
    const stateId = currentStateId();
    el.sessionMeta.textContent = app.sessionId
      ? `Session ${app.sessionId}${sessionModeText() ? ` / ${sessionModeText()}` : ""}${stateId ? ` / state ${stateId}` : ""}`
      : "No session";
    el.reloadButton.disabled = !app.sessionId || app.inFlight;
    el.searchButton.disabled = !app.sessionId || app.inFlight || !isCombatSession();
    el.newSessionButton.disabled = app.inFlight;
    el.sessionModeSelect.disabled = app.inFlight;
    el.refreshBridgeButton.disabled = app.inFlight;
    el.requestBridgeStateButton.disabled = app.inFlight || (app.bridge && app.bridge.pending_command);

    el.stateSummary.textContent = summarizeState();
    el.lifecycleBadge.textContent = lifecycleText();
    el.lifecycleBadge.className = `status-badge ${lifecycleClass()}`;

    if (app.lastError) {
      el.actionError.textContent = app.lastError;
      el.actionError.classList.remove("hidden");
    } else {
      el.actionError.textContent = "";
      el.actionError.classList.add("hidden");
    }

    if (app.inFlight) {
      el.pendingMessage.textContent = lifecycleText();
      el.pendingMessage.classList.remove("hidden");
    } else {
      el.pendingMessage.textContent = "";
      el.pendingMessage.classList.add("hidden");
    }
  }

  function renderBoard() {
    const board = boardState();
    const player = board.player || app.state && app.state.player || null;
    const monsters = arrayOf(board.monsters || app.state && app.state.monsters);

    clear(el.playerPanel);
    if (!player) {
      empty(el.playerPanel, "Player state will appear here.");
    } else {
      el.playerPanel.className = "player-panel";
      el.playerPanel.append(
        statBlock("Player", [
          ["HP", hpText(player)],
          ["Block", firstDefined(player.block, player.current_block, 0)],
          ["Energy", energyText(board, player)],
          ["Status", statusText(player)],
        ]),
      );
    }

    clear(el.monsterPanel);
    if (!monsters.length) {
      empty(el.monsterPanel, "No monsters loaded.");
    } else {
      el.monsterPanel.className = "monster-panel monster-list";
      monsters.forEach((monster, index) => {
        const node = document.createElement("article");
        node.className = monsterAlive(monster) ? "monster" : "monster defeated";
        node.append(
          line("h3", monster.name || monster.label || `Monster ${index + 1}`),
          statList([
            ["ID", firstDefined(monster.id, monster.monster_id, "-")],
            ["HP", hpText(monster)],
            ["Block", firstDefined(monster.block, monster.current_block, 0)],
            ["Intent", intentText(monster)],
          ]),
        );
        el.monsterPanel.appendChild(node);
      });
    }

    const piles = pilesState();
    el.energyStat.textContent = firstDefined(board.energy, board.energy_current, player && player.energy, "-");
    el.drawStat.textContent = countOf(piles.draw || piles.draw_pile || board.draw_pile);
    el.discardStat.textContent = countOf(piles.discard || piles.discard_pile || board.discard_pile);
    el.exhaustStat.textContent = countOf(piles.exhaust || piles.exhaust_pile || board.exhaust_pile);
  }

  function renderHand() {
    const hand = handCards();
    el.handCount.textContent = `${hand.length} ${hand.length === 1 ? "card" : "cards"}`;
    clear(el.handPanel);
    if (!hand.length) {
      empty(el.handPanel, "No hand data.");
      return;
    }
    el.handPanel.className = "hand-row";
    hand.forEach((card, index) => {
      const node = document.createElement("article");
      node.className = "hand-card";
      node.append(
        line("h3", card.name || card.label || `Card ${index + 1}`),
        statList([
          ["Slot", firstDefined(card.hand_slot, card.slot, index)],
          ["ID", firstDefined(card.id, card.card_id, "-")],
          ["Cost", firstDefined(card.cost, card.energy_cost, "-")],
          ["Type", firstDefined(card.type, card.card_type, "-")],
        ]),
      );
      el.handPanel.appendChild(node);
    });
  }

  function renderActions() {
    const actions = app.actions.length ? app.actions : app.lastError ? app.lastActions : [];
    el.actionCount.textContent = `${actions.length} ${actions.length === 1 ? "available" : "available"}`;
    clear(el.actionsPanel);
    if (!actions.length) {
      const reason = emptyActionReason();
      empty(el.actionsPanel, reason);
      return;
    }

    el.actionsPanel.className = "button-grid";
    actions.forEach((action) => {
      const button = document.createElement("button");
      const disabledReason = action.disabled_reason || action.disabledReason;
      button.type = "button";
      button.className = "action-button";
      button.disabled = app.inFlight || action.enabled === false;
      button.textContent = actionLabel(action);
      button.title = disabledReason || sourceTitle(action);
      button.addEventListener("click", () => submitAction(action));
      if (disabledReason) {
        const reason = document.createElement("span");
        reason.className = "button-reason";
        reason.textContent = disabledReason;
        button.appendChild(reason);
      }
      el.actionsPanel.appendChild(button);
    });
  }

  function renderSearch() {
    const combatSearch = isCombatSession();
    el.searchButton.disabled = !app.sessionId || app.inFlight || !combatSearch;
    const best = app.search && app.search.bestAction;
    el.applyBestButton.disabled = !best || app.inFlight || !combatSearch;
    el.searchStatus.textContent = !combatSearch
      ? "Combat only"
      : app.inFlight && app.lifecycle.label === "Searching" ? "Running" : app.search ? "Ready" : "Idle";
    clear(el.searchResult);

    if (!combatSearch) {
      empty(el.searchResult, "Search is combat-only for run sessions.");
      return;
    }

    if (!app.search) {
      empty(el.searchResult, "No recommendation yet.");
      return;
    }

    el.searchResult.className = "search-result";
    el.searchResult.append(
      statBlock("Recommendation", [
        ["Best", best ? actionLabel(best) : "None"],
        ["Value", firstDefined(app.search.value, "-")],
        ["Visits", firstDefined(app.search.visits, "-")],
        ["Win", percentText(app.search.win_probability)],
      ]),
    );

    const pv = arrayOf(app.search.principal_variation);
    if (pv.length) {
      const list = document.createElement("ol");
      list.className = "pv-list";
      pv.forEach((item) => {
        const li = document.createElement("li");
        li.textContent = actionLabel(item);
        list.appendChild(li);
      });
      el.searchResult.appendChild(list);
    }
  }

  function renderBridge() {
    clear(el.bridgePanel);
    if (!app.bridge) {
      empty(el.bridgePanel, "Bridge status not loaded.");
      return;
    }
    el.bridgePanel.className = "bridge-panel";
    const state = app.bridge.connected
      ? app.bridge.stale
        ? "Connected / stale"
        : "Connected"
      : app.bridge.exited
        ? "Exited"
        : "Disconnected";
    el.bridgePanel.append(
      statBlock("CommunicationMod", [
        ["State", state],
        ["Step", firstDefined(app.bridge.last_state_step, "-")],
        ["Ready", firstDefined(app.bridge.ready_for_command, "-")],
        ["Pending command", app.bridge.pending_command ? "Yes" : "No"],
        ["Trace", firstDefined(app.bridge.trace_path, "-")],
      ]),
    );
    const commands = arrayOf(app.bridge.available_commands);
    if (commands.length) {
      const list = document.createElement("div");
      list.className = "command-list";
      commands.forEach((command) => {
        const badge = document.createElement("span");
        badge.textContent = command;
        list.appendChild(badge);
      });
      el.bridgePanel.appendChild(list);
    }
    el.bridgePanel.appendChild(bridgeActionsSection());
    if (app.bridge.last_error || app.bridge.error) {
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = firstDefined(app.bridge.last_error, app.bridge.error);
      el.bridgePanel.appendChild(msg);
    }
    if (app.parity) {
      el.bridgePanel.append(
        statBlock("Parity", [
          ["Status", app.parity.status || "unknown"],
          ["Observed step", firstDefined(app.parity.observed_step, "-")],
          ["Diffs", arrayOf(app.parity.diffs).length],
        ]),
      );
      const diffs = arrayOf(app.parity.diffs);
      if (diffs.length) {
        const list = document.createElement("ul");
        list.className = "diff-list";
        diffs.slice(0, 8).forEach((diff) => {
          const item = document.createElement("li");
          item.textContent = `${diff.path}: sim ${stringify(diff.simulator)} / observed ${stringify(diff.observed)}`;
          list.appendChild(item);
        });
        el.bridgePanel.appendChild(list);
      }
    }
  }

  function bridgeActionsSection() {
    const wrapper = document.createElement("div");
    wrapper.className = "bridge-action-section";
    wrapper.appendChild(line("h3", "Bridge Actions"));

    const actions = arrayOf(app.bridge && app.bridge.bridge_actions);
    if (!actions.length) {
      const emptyText = document.createElement("span");
      emptyText.className = "bridge-action-empty";
      emptyText.textContent = "No bridge actions available.";
      wrapper.appendChild(emptyText);
      return wrapper;
    }

    const grid = document.createElement("div");
    grid.className = "bridge-action-grid";
    actions.forEach((action) => {
      const button = document.createElement("button");
      const disabledReason = action.disabled_reason || action.disabledReason;
      button.type = "button";
      button.className = "bridge-action-button";
      button.disabled = app.inFlight || app.bridge.pending_command || action.enabled === false;
      button.textContent = bridgeActionLabel(action);
      button.title = disabledReason || (app.bridge.pending_command ? "Bridge command pending." : stringify(firstDefined(action.command, action.action_id, action.actionId, "")));
      button.addEventListener("click", () => submitBridgeAction(action));
      if (disabledReason) {
        const reason = document.createElement("span");
        reason.className = "button-reason";
        reason.textContent = disabledReason;
        button.appendChild(reason);
      }
      grid.appendChild(button);
    });
    wrapper.appendChild(grid);
    return wrapper;
  }

  function renderTrace() {
    const selected = selectedTraceMetadata();
    syncTraceSelect();
    el.refreshTracesButton.disabled = app.traceLoading;
    el.loadTraceButton.disabled = app.traceLoading || !app.selectedTraceId;
    el.traceStatus.textContent = traceStatusText();

    if (app.traceError) {
      el.traceError.textContent = app.traceError;
      el.traceError.classList.remove("hidden");
    } else {
      el.traceError.textContent = "";
      el.traceError.classList.add("hidden");
    }

    clear(el.traceMetaPanel);
    if (!selected && !app.traceDetail) {
      empty(el.traceMetaPanel, "Refresh traces to inspect recorded commands.");
    } else {
      const trace = app.traceDetail || selected;
      el.traceMetaPanel.className = "trace-meta";
      el.traceMetaPanel.append(
        statBlock(trace.name || trace.id || "Trace", [
          ["Records", firstDefined(trace.records, "-")],
          ["States", firstDefined(trace.states, "-")],
          ["Actions", firstDefined(trace.actions, "-")],
          ["Errors", firstDefined(trace.parse_errors, trace.parseErrors, 0)],
          ["Steps", stepRangeText(trace)],
          ["Modified", dateText(trace.modified_at || trace.modifiedAt)],
          ["Size", bytesText(trace.bytes)],
        ]),
      );
      const summary = firstDefined(trace.summary, "");
      if (summary) {
        const node = document.createElement("p");
        node.className = "trace-summary";
        node.textContent = stringify(summary);
        el.traceMetaPanel.appendChild(node);
      }
    }

    clear(el.traceRecordsPanel);
    if (app.traceLoading && !app.traceRecords.length) {
      empty(el.traceRecordsPanel, "Loading trace data.");
      return;
    }
    if (!app.traceRecords.length) {
      empty(el.traceRecordsPanel, app.selectedTraceId ? "No records loaded for this page." : "No trace records loaded.");
      return;
    }

    el.traceRecordsPanel.className = "trace-records";
    app.traceRecords.forEach((record) => {
      el.traceRecordsPanel.appendChild(traceRecordRow(record));
    });
  }

  function renderDebug() {
    el.debugTabs.forEach((button) => {
      button.classList.toggle("active", button.dataset.debugTab === app.activeDebugTab);
    });
    el.debugSessionId.textContent = app.sessionId || "-";
    el.debugMode.textContent = sessionModeText() || "-";
    el.debugStateKind.textContent = stateKindText() || "-";
    el.debugStateId.textContent = currentStateId() || "-";
    el.debugPhase.textContent = sessionPhaseText() || "-";
    el.debugDecision.textContent = decisionText() || "-";
    el.debugUnsupported.textContent = unsupportedText() || "-";
    el.debugHash.textContent = firstDefined(app.state && app.state.snapshot_hash, app.state && app.state.hash, app.snapshot && app.snapshot.snapshot_hash, "-");

    const payload = app.activeDebugTab === "snapshot"
      ? app.snapshot || {}
      : app.activeDebugTab === "raw"
        ? {
            state: app.state,
            actions: app.actions,
            search: app.search,
            bridge: app.bridge,
            parity: app.parity,
            traces: app.traces,
            selectedTrace: app.traceDetail,
            traceRecords: app.traceRecords,
            lifecycle: app.lifecycle,
            error: app.lastError,
          }
        : app.state || {};
    el.debugJson.textContent = JSON.stringify(payload, null, 2);
  }

  function normalizeSearch(payload) {
    const recommendation = payload && (payload.recommendation || payload);
    const bestActionId = firstDefined(recommendation.best_action_id, recommendation.bestActionId);
    const currentBest = bestActionId
      ? app.actions.find((action) => action.action_id === bestActionId || action.id === bestActionId)
      : null;
    return {
      bestAction: firstDefined(currentBest, recommendation.best_action, recommendation.bestAction),
      principal_variation: firstDefined(recommendation.principal_variation, recommendation.principalVariation, []),
      visits: recommendation.visits,
      value: recommendation.value,
      win_probability: recommendation.win_probability,
      diagnostics: recommendation.diagnostics,
    };
  }

  function lifecycleFromPayload(lifecycle) {
    if (!lifecycle || !lifecycle.status) return app.lifecycle || { kind: "Ready" };
    const status = String(lifecycle.status).toLowerCase();
    if (status === "applied") {
      return { kind: "Applied", stateId: firstDefined(lifecycle.resulting_state_id, lifecycle.resultingStateId) };
    }
    if (status === "stale") {
      return { kind: "Stale", error: lifecycle.error };
    }
    if (status === "rejected") {
      return { kind: "Rejected", error: lifecycle.error };
    }
    return { kind: "Ready" };
  }

  function currentStateId() {
    return firstDefined(
      app.state && app.state.state_id,
      app.state && app.state.id,
      app.state && app.state.snapshot_hash,
      app.snapshot && app.snapshot.state_id,
      app.snapshot && app.snapshot.snapshot_hash,
      null,
    );
  }

  function syncTraceSelect() {
    const selected = app.selectedTraceId;
    clear(el.traceSelect);
    if (!app.traces.length) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = app.traceLoading ? "Loading traces..." : "No traces loaded";
      el.traceSelect.appendChild(option);
      el.traceSelect.value = "";
      return;
    }

    app.traces.forEach((trace) => {
      const option = document.createElement("option");
      option.value = String(trace.id);
      option.textContent = traceOptionLabel(trace);
      el.traceSelect.appendChild(option);
    });
    el.traceSelect.value = selected;
  }

  function selectedTraceMetadata() {
    return app.traces.find((trace) => String(trace.id) === app.selectedTraceId) || null;
  }

  function traceStatusText() {
    if (app.traceLoading) return "Loading";
    if (app.traceError) return "Error";
    if (app.traceRecords.length) return `${app.traceRecords.length} record${app.traceRecords.length === 1 ? "" : "s"} shown`;
    if (app.traces.length) return `${app.traces.length} trace${app.traces.length === 1 ? "" : "s"} available`;
    return "Not loaded";
  }

  function traceOptionLabel(trace) {
    const name = firstDefined(trace.name, trace.id, "Trace");
    const records = firstDefined(trace.records, null);
    const modified = dateText(trace.modified_at || trace.modifiedAt);
    const suffix = [
      records === null ? "" : `${records} rec`,
      modified === "-" ? "" : modified,
    ].filter(Boolean).join(" / ");
    return suffix ? `${name} (${suffix})` : name;
  }

  function traceRecordRow(record) {
    const row = document.createElement("article");
    row.className = "trace-record";

    const main = document.createElement("div");
    main.className = "trace-record-main";
    main.append(
      traceChip(firstDefined(record.type, "record")),
      traceCell("Line", firstDefined(record.line, "-")),
      traceCell("Step", firstDefined(record.step, "-")),
      traceCell("Time", timeText(record.timestamp)),
    );

    const body = document.createElement("div");
    body.className = "trace-record-body";
    const title = document.createElement("strong");
    title.textContent = traceRecordTitle(record);
    const summary = document.createElement("span");
    summary.textContent = traceSummaryText(record.summary);
    body.append(title);
    if (summary.textContent && summary.textContent !== title.textContent) {
      body.appendChild(summary);
    }

    row.append(main, body);
    return row;
  }

  function traceRecordTitle(record) {
    if (record.command) return record.command;
    const summary = record.summary || {};
    if (record.type === "state") {
      return [
        firstDefined(summary.screen_type, summary.room_phase, "State"),
        summary.floor === undefined || summary.floor === null ? "" : `floor ${summary.floor}`,
        summary.hp ? `HP ${summary.hp}` : "",
      ].filter(Boolean).join(" / ");
    }
    if (record.type === "metadata") {
      return firstDefined(summary.event, summary.source, "Trace metadata");
    }
    return humanize(firstDefined(record.type, "Trace record"));
  }

  function traceSummaryText(summary) {
    if (!summary || typeof summary !== "object") return stringify(firstDefined(summary, ""));
    const parts = [];
    if (summary.ready_for_command !== undefined) parts.push(`ready ${summary.ready_for_command}`);
    if (summary.available_commands && summary.available_commands.length) {
      parts.push(`commands ${summary.available_commands.join(", ")}`);
    }
    if (summary.choices && summary.choices.length) {
      parts.push(`choices ${summary.choices.join(", ")}`);
    }
    if (summary.combat) {
      if (summary.combat.energy !== undefined && summary.combat.energy !== null) {
        parts.push(`energy ${summary.combat.energy}`);
      }
      if (summary.combat.hand && summary.combat.hand.length) {
        parts.push(`hand ${summary.combat.hand.join(", ")}`);
      }
    }
    if (!parts.length) return stringify(summary);
    return parts.join(" | ");
  }

  function traceChip(value) {
    const node = document.createElement("span");
    node.className = "trace-chip";
    node.textContent = humanize(value);
    return node;
  }

  function traceCell(label, value) {
    const node = document.createElement("span");
    node.className = "trace-cell";
    node.textContent = `${label} ${stringify(value)}`;
    return node;
  }

  function summarizeState() {
    if (!app.state) return "Start a fixture to inspect simulator state.";
    const kind = stateKindText();
    const phase = sessionPhaseText() || "combat";
    const terminal = firstDefined(app.state.terminal_reason, app.state.terminalReason, null);
    const unsupported = unsupportedText();
    const decision = decisionText();
    if (terminal) return `${phase}: ${terminal}`;
    if (unsupported) return `${kind ? `${kind} / ` : ""}${phase}: unsupported ${unsupported}`;
    return [
      kind || "",
      phase,
      decision ? `decision ${decision}` : "",
      `${app.actions.length} legal action${app.actions.length === 1 ? "" : "s"}`,
    ].filter(Boolean).join(" / ");
  }

  function lifecycleText() {
    if (!app.lifecycle) return "Ready";
    if (app.lifecycle.kind === "Submitting") return app.lifecycle.label || "Submitting";
    if (app.lifecycle.kind === "Applied") return app.lifecycle.stateId ? `Applied to ${app.lifecycle.stateId}` : "Applied";
    if (app.lifecycle.kind === "Rejected") return "Rejected";
    if (app.lifecycle.kind === "Stale") return "Stale";
    return "Ready";
  }

  function lifecycleClass() {
    if (!app.lifecycle) return "neutral";
    if (app.lifecycle.kind === "Rejected" || app.lifecycle.kind === "Stale") return "bad";
    if (app.lifecycle.kind === "Submitting") return "busy";
    if (app.lifecycle.kind === "Applied") return "good";
    return "neutral";
  }

  function boardState() {
    return firstDefined(app.state && app.state.board, app.state && app.state.combat, {});
  }

  function pilesState() {
    const board = boardState();
    return firstDefined(board.piles, app.state && app.state.piles, {});
  }

  function handCards() {
    const board = boardState();
    const piles = pilesState();
    return arrayOf(firstDefined(board.hand, piles.hand, app.state && app.state.hand));
  }

  function emptyActionReason() {
    const terminal = app.state && firstDefined(app.state.terminal_reason, app.state.terminalReason);
    const unsupported = unsupportedText();
    const waiting = app.state && firstDefined(app.state.waiting_for_bridge, app.state.waitingForBridge);
    const internal = app.state && firstDefined(app.state.internal_error, app.state.internalError);
    if (terminal) return `Terminal: ${terminal}`;
    if (unsupported) return `Unsupported: ${unsupported}`;
    if (waiting) return "Waiting for bridge state.";
    if (internal) return `Internal error: ${internal}`;
    if (app.sessionId) return "No actions returned. The service should report terminal, unsupported, waiting, stale, or internal-error state.";
    return "No actions loaded.";
  }

  function actionLabel(action) {
    if (!action) return "Unknown action";
    const descriptors = firstDefined(action.descriptors, action.ui_descriptors, action.uiDescriptors);
    if (Array.isArray(descriptors) && descriptors.length) {
      return descriptors.map((descriptor) => descriptorLabel(descriptor)).join(" / ");
    }
    const descriptor = firstDefined(
      action.label,
      action.descriptor,
      action.ui_action,
      action.action,
      action.kind,
      action.type,
      action.name,
      action.exact_action_json,
      action.exactActionJson,
    );
    return descriptorLabel(descriptor, firstDefined(action.action_id, action.id, "Action"));
  }

  function descriptorLabel(descriptor, fallback) {
    if (typeof descriptor === "string") return descriptor;
    if (descriptor && typeof descriptor === "object") {
      const kind = firstDefined(descriptor.label, descriptor.kind, descriptor.type, descriptor.name, fallback || "Action");
      const details = Object.entries(descriptor)
        .filter(([key]) => !["label", "kind", "type", "name"].includes(key))
        .map(([key, value]) => `${humanize(key)} ${stringify(value)}`)
        .join(", ");
      return details ? `${humanize(kind)} (${details})` : humanize(kind);
    }
    return humanize(firstDefined(fallback, "Action"));
  }

  function bridgeActionLabel(action) {
    if (!action) return "Bridge action";
    return stringify(firstDefined(action.label, action.command, action.action_id, action.actionId, "Bridge action"));
  }

  function sourceTitle(action) {
    const source = firstDefined(action.source_state_id, action.sourceStateId, currentStateId(), "-");
    const exactActionJson = firstDefined(action.exact_action_json, action.exactActionJson, "");
    const exact = exactActionJson ? ` / ${stringify(exactActionJson)}` : "";
    return `Derived from state ${source}${exact}`;
  }

  function isCombatSession() {
    return stateKindText() !== "run" || sessionPhaseText() === "combat";
  }

  function sessionModeText() {
    return firstDefined(app.mode, app.state && app.state.mode, "");
  }

  function stateKindText() {
    return firstDefined(app.stateKind, app.state && app.state.state_kind, app.state && app.state.stateKind, "");
  }

  function sessionPhaseText() {
    return firstDefined(app.phase, app.state && app.state.phase, app.state && app.state.decision_substate, "");
  }

  function decisionText() {
    const decision = firstDefined(
      app.currentDecision,
      app.state && app.state.current_decision,
      app.state && app.state.currentDecision,
      app.state && app.state.decision,
      app.state && app.state.decision_substate,
    );
    if (!decision) return "";
    if (typeof decision === "string") return decision;
    return stringify(firstDefined(decision.label, decision.kind, decision.type, decision.name, decision));
  }

  function unsupportedText() {
    const unsupported = firstDefined(
      app.unsupportedReason,
      app.state && app.state.unsupported_reason,
      app.state && app.state.unsupportedReason,
      app.state && app.state.unsupported_decision,
      app.state && app.state.unsupportedDecision,
    );
    return unsupported ? stringify(unsupported) : "";
  }

  function showError(message) {
    app.lastError = message;
    app.lifecycle = { kind: "Rejected", error: message };
    render();
  }

  function flashPending() {
    el.pendingMessage.classList.add("flash");
    window.setTimeout(() => el.pendingMessage.classList.remove("flash"), 260);
  }

  function readableError(error) {
    if (!error) return "Unknown error.";
    return errorFromPayload(error.payload) || error.message || String(error);
  }

  function isStaleError(error) {
    const text = readableError(error).toLowerCase();
    return error.status === 409 || text.includes("stale") || text.includes("source_state_id");
  }

  function errorFromPayload(payload) {
    if (!payload) return "";
    if (typeof payload === "string") return payload;
    return firstDefined(payload.public_error, payload.error, payload.message, payload.detail, "");
  }

  function statBlock(title, rows) {
    const wrapper = document.createElement("div");
    wrapper.className = "stat-block";
    wrapper.append(line("h3", title), statList(rows));
    return wrapper;
  }

  function statList(rows) {
    const dl = document.createElement("dl");
    dl.className = "stat-list";
    rows.forEach(([label, value]) => {
      const group = document.createElement("div");
      const dt = document.createElement("dt");
      const dd = document.createElement("dd");
      dt.textContent = label;
      dd.textContent = stringify(value);
      group.append(dt, dd);
      dl.appendChild(group);
    });
    return dl;
  }

  function line(tag, text) {
    const node = document.createElement(tag);
    node.textContent = stringify(text);
    return node;
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
    node.classList.remove("empty");
  }

  function empty(node, text) {
    node.classList.add("empty");
    const span = document.createElement("span");
    span.className = "empty-text";
    span.textContent = text;
    node.appendChild(span);
  }

  function firstDefined(...values) {
    for (const value of values) {
      if (value !== undefined && value !== null) return value;
    }
    return undefined;
  }

  function arrayOf(value) {
    return Array.isArray(value) ? value : [];
  }

  function countOf(value) {
    if (Array.isArray(value)) return value.length;
    if (typeof value === "number") return value;
    if (value && typeof value.count === "number") return value.count;
    return "-";
  }

  function boundedInteger(value, fallback, min, max) {
    const parsed = Number.parseInt(value, 10);
    if (!Number.isFinite(parsed)) return fallback;
    return Math.min(max, Math.max(min, parsed));
  }

  function bytesText(value) {
    if (typeof value !== "number") return "-";
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }

  function dateText(value) {
    if (!value) return "-";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return stringify(value);
    return date.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function timeText(value) {
    if (!value) return "-";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return stringify(value);
    return date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  function stepRangeText(trace) {
    const first = firstDefined(trace.first_step, trace.firstStep, null);
    const last = firstDefined(trace.last_step, trace.lastStep, null);
    if (first === null && last === null) return "-";
    if (first === last) return stringify(first);
    return `${stringify(first)}-${stringify(last)}`;
  }

  function hpText(entity) {
    const current = firstDefined(entity.current_hp, entity.hp, entity.health, "?");
    const max = firstDefined(entity.max_hp, entity.maxHealth, entity.max_health, null);
    return max === null ? current : `${current}/${max}`;
  }

  function energyText(board, player) {
    const current = firstDefined(board.energy, board.energy_current, player.energy, "?");
    const max = firstDefined(board.max_energy, player.max_energy, player.energy_per_turn, null);
    return max === null ? current : `${current}/${max}`;
  }

  function statusText(entity) {
    const statuses = arrayOf(firstDefined(entity.powers, entity.statuses, entity.buffs));
    if (!statuses.length) return "None";
    return statuses.map((status) => status.name || status.id || String(status)).join(", ");
  }

  function intentText(monster) {
    const intent = firstDefined(monster.intent, monster.move, monster.next_move, "Unknown");
    if (typeof intent === "string") return intent;
    return firstDefined(intent.name, intent.intent, intent.type, JSON.stringify(intent));
  }

  function monsterAlive(monster) {
    if (monster.is_gone || monster.gone || monster.dead) return false;
    const hp = firstDefined(monster.current_hp, monster.hp, monster.health, 1);
    return hp > 0;
  }

  function percentText(value) {
    if (typeof value !== "number") return "-";
    return `${Math.round(value * 100)}%`;
  }

  function humanize(value) {
    return stringify(value)
      .replace(/[_-]+/g, " ")
      .replace(/\b\w/g, (letter) => letter.toUpperCase());
  }

  function stringify(value) {
    if (value === undefined || value === null) return "-";
    if (typeof value === "object") return JSON.stringify(value);
    return String(value);
  }
})();
