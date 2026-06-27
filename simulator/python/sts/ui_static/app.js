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
    activeDebugTab: "state",
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
      "debugSessionId",
      "debugStateId",
      "debugPhase",
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
    el.debugTabs.forEach((button) => {
      button.addEventListener("click", () => {
        app.activeDebugTab = button.dataset.debugTab;
        renderDebug();
      });
    });
  }

  async function startSession() {
    await singleFlight("Starting fixture", async () => {
      const session = await requestJson("/api/sessions", {
        method: "POST",
        body: { mode: "combat_fixture" },
      });
      adoptSession(session);
      app.snapshot = null;
      app.search = null;
      await loadSnapshotQuietly();
    });
  }

  async function reloadSession() {
    if (!app.sessionId) return;
    await singleFlight("Reloading session", async () => {
      const session = await requestJson(`/api/sessions/${encodeURIComponent(app.sessionId)}`);
      adoptSession(session);
      await loadSnapshotQuietly();
    });
  }

  async function runSearch() {
    if (!app.sessionId) return;
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

  function adoptSession(payload) {
    const state = payload && (payload.state || payload.ui_state || payload);
    app.sessionId = firstDefined(payload && payload.id, payload && payload.session_id, state && state.session_id, app.sessionId);
    app.state = state || null;
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
  }

  function render() {
    renderChrome();
    renderBoard();
    renderHand();
    renderActions();
    renderSearch();
    renderDebug();
  }

  function renderChrome() {
    const stateId = currentStateId();
    el.sessionMeta.textContent = app.sessionId
      ? `Session ${app.sessionId}${stateId ? ` / state ${stateId}` : ""}`
      : "No session";
    el.reloadButton.disabled = !app.sessionId || app.inFlight;
    el.searchButton.disabled = !app.sessionId || app.inFlight;
    el.newSessionButton.disabled = app.inFlight;

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
    el.searchButton.disabled = !app.sessionId || app.inFlight;
    const best = app.search && app.search.bestAction;
    el.applyBestButton.disabled = !best || app.inFlight;
    el.searchStatus.textContent = app.inFlight && app.lifecycle.label === "Searching" ? "Running" : app.search ? "Ready" : "Idle";
    clear(el.searchResult);

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

  function renderDebug() {
    el.debugTabs.forEach((button) => {
      button.classList.toggle("active", button.dataset.debugTab === app.activeDebugTab);
    });
    el.debugSessionId.textContent = app.sessionId || "-";
    el.debugStateId.textContent = currentStateId() || "-";
    el.debugPhase.textContent = firstDefined(app.state && app.state.phase, app.state && app.state.decision_substate, "-");
    el.debugHash.textContent = firstDefined(app.state && app.state.snapshot_hash, app.state && app.state.hash, app.snapshot && app.snapshot.snapshot_hash, "-");

    const payload = app.activeDebugTab === "snapshot"
      ? app.snapshot || {}
      : app.activeDebugTab === "raw"
        ? { state: app.state, actions: app.actions, search: app.search, lifecycle: app.lifecycle, error: app.lastError }
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

  function summarizeState() {
    if (!app.state) return "Start a combat fixture to inspect simulator state.";
    const phase = firstDefined(app.state.phase, app.state.decision_substate, "combat");
    const terminal = firstDefined(app.state.terminal_reason, app.state.terminalReason, null);
    if (terminal) return `${phase}: ${terminal}`;
    return `${phase} / ${app.actions.length} legal action${app.actions.length === 1 ? "" : "s"}`;
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
    const unsupported = app.state && firstDefined(app.state.unsupported_decision, app.state.unsupportedDecision);
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
    const descriptor = firstDefined(action.descriptor, action.ui_action, action.action, action.kind, action.type, action.name);
    if (typeof descriptor === "string") return descriptor;
    if (descriptor && typeof descriptor === "object") {
      const kind = firstDefined(descriptor.kind, descriptor.type, descriptor.name, "Action");
      const details = Object.entries(descriptor)
        .filter(([key]) => !["kind", "type", "name"].includes(key))
        .map(([key, value]) => `${humanize(key)} ${value}`)
        .join(", ");
      return details ? `${humanize(kind)} (${details})` : humanize(kind);
    }
    return humanize(firstDefined(action.action_id, action.id, "Action"));
  }

  function sourceTitle(action) {
    const source = firstDefined(action.source_state_id, action.sourceStateId, currentStateId(), "-");
    return `Derived from state ${source}`;
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
