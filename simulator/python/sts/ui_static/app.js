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
    bridgeClients: [],
    bridgeClientsError: null,
    bridgeClientsLoading: false,
    bridgeIdentity: null,
    bridgeIdentityWarning: null,
    parity: null,
    traces: [],
    selectedTraceId: "",
    traceDetail: null,
    traceRecords: [],
    traceError: null,
    traceLoading: false,
    activeDebugTab: "state",
    viewMode: "live",
    bridgePollTimer: null,
    liveAutoAttachInFlight: false,
    liveBridgeStateId: null,
    liveBridgeStep: null,
    liveSearchBridgeStateId: null,
    liveSendAction: null,
    livePendingPrediction: null,
    livePendingPlanIndex: null,
    liveAutoPlayPlan: false,
    liveInvariantViolation: null,
    collector: null,
    collectorLastError: null,
    collectorAutoRun: false,
    collectorAutoTimer: null,
    collectorReport: null,
    slaythedataCandidates: [],
    slaythedataSelectedRunId: "",
    slaythedataStatus: null,
    slaythedataLastError: null,
    attachFidelity: null,
    strictReplayBlocker: null,
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
    refreshBridgeQuietly().finally(() => render());
    refreshBridgeClientsQuietly().finally(() => renderBridgeClients());
    refreshCollectorQuietly().finally(() => renderCollector());
    refreshCollectorReportQuietly().finally(() => renderCollectorReport());
    refreshSlaythedataStatusQuietly().finally(() => renderCollector());
    startBridgePolling();
    render();
  });

  function bindElements() {
    for (const id of [
      "sessionMeta",
      "liveModeButton",
      "simModeButton",
      "fixtureControls",
      "liveBand",
      "liveSummary",
      "liveStatusBadge",
      "liveStatusPanel",
      "startCharacterSelect",
      "startAscensionInput",
      "startSeedInput",
      "startLiveRunButton",
      "attachLiveButton",
      "liveSearchButton",
      "sendBestButton",
      "sendBestReviewButton",
      "liveWorkflow",
      "liveReason",
      "sessionModeSelect",
      "newSessionButton",
      "reloadButton",
      "stateSummary",
      "lifecycleBadge",
      "actionsTitle",
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
      "searchPolicySelect",
      "maxDepthInput",
      "allowedPotionsPanel",
      "searchButton",
      "applyBestButton",
      "searchStatus",
      "searchResult",
      "refreshBridgeButton",
      "liveRequestStateButton",
      "collectorPayloadInput",
      "slaythedataCandidateSelect",
      "findSlaythedataRunsButton",
      "loadSlaythedataRunButton",
      "startCollectorButton",
      "startGuidedLiveRunButton",
      "startGuidedAutoRunButton",
      "previewCollectorButton",
      "sendCollectorButton",
      "autoCollectorButton",
      "pauseCollectorButton",
      "stopCollectorButton",
      "collectorStatusPanel",
      "collectorReportPanel",
      "requestBridgeStateButton",
      "clearOrphanCommandMetaButton",
      "refreshBridgeClientsButton",
      "bridgeClientsPanel",
      "invariantModal",
      "invariantTitle",
      "invariantMessage",
      "invariantFacts",
      "ackInvariantButton",
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
      "restoreSnapshotButton",
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
    el.liveModeButton.addEventListener("click", () => setViewMode("live"));
    el.simModeButton.addEventListener("click", () => setViewMode("sim"));
    el.startLiveRunButton.addEventListener("click", startLiveRun);
    el.attachLiveButton.addEventListener("click", attachLiveSession);
    el.liveSearchButton.addEventListener("click", runLiveSearch);
    el.sendBestButton.addEventListener("click", () => sendBestToGame({ autoPlay: false }));
    el.sendBestReviewButton.addEventListener("click", () => sendBestToGame({ autoPlay: true }));
    el.newSessionButton.addEventListener("click", startSession);
    el.reloadButton.addEventListener("click", reloadSession);
    el.searchButton.addEventListener("click", runSearch);
    el.applyBestButton.addEventListener("click", applyBestAction);
    el.refreshBridgeButton.addEventListener("click", refreshBridge);
    el.liveRequestStateButton.addEventListener("click", requestBridgeState);
    el.startCollectorButton.addEventListener("click", startGuidedCollector);
    el.startGuidedLiveRunButton.addEventListener("click", startGuidedLiveRun);
    el.startGuidedAutoRunButton.addEventListener("click", () => startGuidedLiveRun({ armAuto: true }));
    el.collectorReportPanel.addEventListener("dblclick", () => refreshCollectorReport().catch(showError));
    el.findSlaythedataRunsButton.addEventListener("click", findSlaythedataRuns);
    el.loadSlaythedataRunButton.addEventListener("click", loadSelectedSlaythedataRun);
    el.slaythedataCandidateSelect.addEventListener("change", () => {
      app.slaythedataSelectedRunId = el.slaythedataCandidateSelect.value;
      applySelectedSlaythedataRunToStartControls();
      renderCollector();
    });
    el.previewCollectorButton.addEventListener("click", () => tickGuidedCollector({ send: false }));
    el.sendCollectorButton.addEventListener("click", () => tickGuidedCollector({ send: true }));
    el.autoCollectorButton.addEventListener("click", startCollectorAutoRun);
    el.pauseCollectorButton.addEventListener("click", pauseCollectorAutoRun);
    el.stopCollectorButton.addEventListener("click", stopGuidedCollector);
    el.requestBridgeStateButton.addEventListener("click", requestBridgeState);
    el.clearOrphanCommandMetaButton.addEventListener("click", clearOrphanCommandMetadata);
    el.refreshBridgeClientsButton.addEventListener("click", (event) => {
      event.stopPropagation();
      refreshBridgeClients();
    });
    el.ackInvariantButton.addEventListener("click", acknowledgeInvariantViolation);
    el.refreshTracesButton.addEventListener("click", refreshTraces);
    el.loadTraceButton.addEventListener("click", loadSelectedTrace);
    el.restoreSnapshotButton.addEventListener("click", restoreSnapshot);
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

  function setViewMode(mode) {
    app.viewMode = mode === "sim" ? "sim" : "live";
    render();
  }

  function startBridgePolling() {
    if (app.bridgePollTimer) return;
    app.bridgePollTimer = window.setInterval(async () => {
      if (app.inFlight) return;
      const previousStateId = bridgeStateId();
      await refreshBridgeQuietly();
      const currentBridgeStateId = bridgeStateId();
      if (app.viewMode === "live" && currentBridgeStateId && currentBridgeStateId !== previousStateId) {
        await autoAttachLiveStateQuietly();
      }
      await refreshCollectorQuietly();
      if (currentBridgeStateId !== previousStateId || app.viewMode === "live") {
        renderChrome();
        renderLive();
        renderBoard();
        renderHand();
        renderActions();
        renderAllowedPotions();
        renderCollector();
        renderSearch();
        renderBridge();
        renderDebug();
        renderInvariantModal();
      }
    }, 1000);
  }

  async function startLiveRun() {
    const character = (el.startCharacterSelect.value || "IRONCLAD").trim().toUpperCase();
    const ascension = boundedInteger(el.startAscensionInput.value, 0, 0, 20);
    const seed = (el.startSeedInput.value || "").trim();
    if (!seed) {
      showError("Seed is required to start a live run.");
      return;
    }
    await submitBridgeCommand(`START ${character} ${ascension} ${seed}`);
  }

  async function attachLiveSession() {
    await singleFlight("Attaching current state", async () => {
      await refreshBridgeQuietly();
      const session = await requestJson("/api/live/session", { method: "POST", body: {} });
      adoptSession(session);
      app.viewMode = "live";
      app.snapshot = null;
      app.search = null;
      app.liveSendAction = null;
      app.livePendingPrediction = null;
      app.livePendingPlanIndex = null;
      app.liveAutoPlayPlan = false;
      app.liveBridgeStateId = firstDefined(session.bridge_state_id, bridgeStateId(), null);
      app.liveBridgeStep = firstDefined(session.bridge_step, app.bridge && app.bridge.last_state_step, null);
      app.attachFidelity = firstDefined(session.attach_fidelity, session.attachFidelity, app.attachFidelity);
      app.strictReplayBlocker = firstDefined(session.strict_replay_blocker, session.strictReplayBlocker, null);
      app.liveSearchBridgeStateId = null;
      await loadSnapshotQuietly();
      await refreshParityQuietly();
    });
  }

  async function autoAttachLiveStateQuietly() {
    if (app.liveAutoAttachInFlight || app.inFlight || app.viewMode !== "live") return;
    if (!canAttachLiveSession()) return;
    const currentBridgeStateId = bridgeStateId();
    if (!currentBridgeStateId || app.liveBridgeStateId === currentBridgeStateId) return;
    app.liveAutoAttachInFlight = true;
    const previousSearch = app.search;
    const pendingPlanIndex = app.livePendingPlanIndex;
    try {
      const session = await requestJson("/api/live/session", { method: "POST", body: {} });
      adoptSession(session);
      app.viewMode = "live";
      app.snapshot = null;
      app.liveSendAction = null;
      app.liveBridgeStateId = firstDefined(session.bridge_state_id, currentBridgeStateId, null);
      app.liveBridgeStep = firstDefined(session.bridge_step, app.bridge && app.bridge.last_state_step, null);
      app.attachFidelity = firstDefined(session.attach_fidelity, session.attachFidelity, app.attachFidelity);
      app.strictReplayBlocker = firstDefined(session.strict_replay_blocker, session.strictReplayBlocker, null);
      app.liveSearchBridgeStateId = null;
      await loadSnapshotQuietly();
      await refreshParityQuietly();
      if (verifyPendingLivePrediction(currentStateId())) {
        const advanced = advanceLiveSearchPlan(previousSearch, pendingPlanIndex, currentBridgeStateId);
        if (advanced && app.liveAutoPlayPlan) {
          scheduleAutoPlayPlanStep();
        }
      } else if (app.liveInvariantViolation) {
        app.search = null;
        app.livePendingPlanIndex = null;
        app.liveAutoPlayPlan = false;
      }
    } catch (error) {
      app.lastError = readableError(error);
    } finally {
      app.liveAutoAttachInFlight = false;
    }
  }

  async function runLiveSearch() {
    if (!app.sessionId || app.mode !== "live_bridge" || app.liveBridgeStateId !== bridgeStateId()) {
      await attachLiveSession();
    }
    if (!isCombatSession()) {
      showError("Live search is only available in combat.");
      return;
    }
    await runSearch();
    app.liveSearchBridgeStateId = bridgeStateId();
    app.liveSendAction = liveBridgeActionForBest();
    app.livePendingPlanIndex = null;
    renderLive();
  }

  async function sendBestToGame(options = {}) {
    const action = liveBridgeActionForBest();
    if (!action) {
      showError(liveSendBlockedReason() || "Recommendation cannot be sent to the live game.");
      if (options.autoPlay) app.liveAutoPlayPlan = false;
      return;
    }
    app.liveAutoPlayPlan = !!options.autoPlay;
    const prediction = await predictLiveBestAction();
    if (!prediction) {
      if (options.autoPlay) app.liveAutoPlayPlan = false;
      return;
    }
    app.livePendingPrediction = {
      source_state_id: firstDefined(prediction.source_state_id, currentStateId(), null),
      predicted_state_id: firstDefined(prediction.predicted_state_id, prediction.predictedStateId, null),
      bridge_state_id: bridgeStateId(),
      bridge_step: app.bridge && app.bridge.last_state_step,
      action_label: recommendationBestLabel(),
    };
    app.livePendingPlanIndex = nextPrincipalVariationIndex();
    await submitBridgeAction(action);
    app.liveSendAction = null;
  }

  function scheduleAutoPlayPlanStep() {
    window.setTimeout(async () => {
      if (!app.liveAutoPlayPlan || app.livePendingPrediction || app.inFlight) return;
      let blocker = liveSendBlockedReason();
      if (blocker && shouldRefreshAutoPlayRecommendation(blocker)) {
        const recovered = await refreshLiveSearchForCurrentState();
        blocker = recovered ? liveSendBlockedReason() : blocker;
      }
      if (blocker) {
        app.liveAutoPlayPlan = false;
        app.lastError = `Auto-play stopped: ${blocker}`;
        render();
        return;
      }
      await sendBestToGame({ autoPlay: true });
      render();
    }, 150);
  }

  function shouldRefreshAutoPlayRecommendation(blocker) {
    if (!app.liveAutoPlayPlan || app.livePendingPrediction || app.inFlight) return false;
    return blocker === "Run search first."
      || blocker === "Recommendation is for an older bridge state."
      || blocker === "Recommendation cannot be mapped to a current bridge command.";
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

  async function runSearch(options = {}) {
    if (!app.sessionId || !isCombatSession()) return;
    const maxDepth = Number.parseInt(el.maxDepthInput.value, 10);
    const candidate = el.searchPolicySelect.value;
    const allowedPotions = allowedPotionsPayload();
    await singleFlight("Searching", async () => {
      app.search = null;
      renderSearch();
      const recommendation = await requestJson(
        `/api/sessions/${encodeURIComponent(app.sessionId)}/search`,
        {
          method: "POST",
          body: {
            candidate,
            max_depth: Number.isFinite(maxDepth) ? maxDepth : undefined,
            allowed_potions: allowedPotions,
            source_state_id: currentStateId(),
          },
        },
      );
      app.search = normalizeSearch(recommendation);
      app.livePendingPlanIndex = null;
      if (!options.preserveAutoPlay) {
        app.liveAutoPlayPlan = false;
      }
    });
  }

  async function refreshLiveSearchForCurrentState() {
    if (liveSearchBlockedReason()) return false;
    await runSearch({ preserveAutoPlay: true });
    app.liveSearchBridgeStateId = bridgeStateId();
    app.liveSendAction = liveBridgeActionForBest();
    app.livePendingPlanIndex = null;
    return Boolean(app.search && app.search.bestAction && app.liveSendAction);
  }

  async function predictLiveBestAction() {
    const best = app.search && app.search.bestAction;
    if (!app.sessionId || !best) return null;
    try {
      return await requestJson(`/api/sessions/${encodeURIComponent(app.sessionId)}/predict`, {
        method: "POST",
        body: predictionPayload(best),
      });
    } catch (error) {
      showError(`Cannot predict the next simulator state: ${readableError(error)}`);
      return null;
    }
  }

  async function applyBestAction() {
    const best = app.search && app.search.bestAction;
    if (!best) return;
    await submitAction(best);
  }

  async function restoreSnapshot() {
    if (!app.sessionId || app.inFlight) {
      flashPending();
      return;
    }

    const snapshotJson = app.snapshot && app.snapshot.snapshot_json;
    if (!snapshotJson) {
      showError("Cannot restore because no snapshot_json is loaded.");
      return;
    }

    const sourceStateId = currentStateId();
    await singleFlight("Restoring snapshot", async () => {
      const result = await requestJson(`/api/sessions/${encodeURIComponent(app.sessionId)}/restore`, {
        method: "POST",
        body: {
          snapshot_json: snapshotJson,
          source_state_id: sourceStateId || undefined,
        },
      });
      adoptSession(result.session || result.state || result);
      app.search = null;
      await loadSnapshotQuietly();
      await refreshParityQuietly();
      const lifecycle = firstDefined(result.command_lifecycle, result.commandLifecycle, null);
      if (lifecycle && (lifecycle.status === "stale" || lifecycle.status === "rejected")) {
        throw new Error(firstDefined(lifecycle.error, result.last_error, "Snapshot restore was rejected."));
      }
    }, {
      sourceStateId,
      successKind: "Restored",
    });
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
      if (!app.lifecycle || app.lifecycle.kind === "Submitting") {
        app.lifecycle = { kind: pending && pending.successKind || "Applied", stateId: currentStateId() };
      }
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
      const previousIdentity = app.bridgeIdentity;
      app.bridge = await requestJson("/api/bridge");
      const nextIdentity = bridgeIdentity(app.bridge);
      if (
        previousIdentity &&
        nextIdentity &&
        (previousIdentity.clientPid !== nextIdentity.clientPid || previousIdentity.tracePath !== nextIdentity.tracePath)
      ) {
        app.bridgeIdentityWarning = {
          previous: previousIdentity,
          current: nextIdentity,
        };
        app.search = null;
        app.liveSearchBridgeStateId = null;
        app.liveSendAction = null;
        refreshBridgeClientsQuietly().finally(() => renderBridgeClients());
      }
      if (nextIdentity) {
        app.bridgeIdentity = nextIdentity;
      }
    } catch (error) {
      app.bridge = { error: readableError(error), connected: false, stale: true };
    }
  }

  async function refreshCollectorQuietly() {
    try {
      app.collector = await requestJson("/api/collector/status");
      app.collectorLastError = null;
    } catch (error) {
      app.collector = { active: false, status: "error" };
      app.collectorLastError = readableError(error);
    }
  }

  async function refreshCollectorReportQuietly() {
    try {
      app.collectorReport = await requestJson("/api/collector/report");
    } catch (error) {
      app.collectorReport = { ok: false, missing: true, error: readableError(error) };
    }
  }

  async function refreshSlaythedataStatusQuietly() {
    const character = (el.startCharacterSelect && el.startCharacterSelect.value || "IRONCLAD").trim().toUpperCase();
    const ascension = boundedInteger(el.startAscensionInput && el.startAscensionInput.value, 0, 0, 20);
    const params = new URLSearchParams({
      character,
      ascension: String(ascension),
      min_floor: "45",
      min_path_length: "45",
    });
    try {
      app.slaythedataStatus = await requestJson(`/api/slaythedata/status?${params.toString()}`);
      app.slaythedataLastError = null;
    } catch (error) {
      app.slaythedataStatus = null;
      app.slaythedataLastError = readableError(error);
    }
  }

  async function startGuidedCollector() {
    const text = el.collectorPayloadInput.value.trim();
    if (!text) {
      showError("Paste a SlayTheData export row or GuidedRunScript first.");
      return;
    }
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch (error) {
      showError(`Collector JSON is invalid: ${error.message}`);
      return;
    }
    const body = parsed && parsed.schema ? { script: parsed } : { exported_run: parsed };
    await singleFlight("Starting guided collector", async () => {
      pauseCollectorAutoRun();
      app.collector = await requestJson("/api/collector/start", { method: "POST", body });
      applyGuidedRunStartControls(app.collector && app.collector.config);
      app.collectorLastError = null;
    });
  }

  async function findSlaythedataRuns() {
    const character = (el.startCharacterSelect.value || "IRONCLAD").trim().toUpperCase();
    const ascension = boundedInteger(el.startAscensionInput.value, 0, 0, 20);
    const params = new URLSearchParams({
      character,
      ascension: String(ascension),
      min_floor: "45",
      max_floor: "55",
      min_path_length: "45",
      min_card_choices: "8",
      min_event_choices: "1",
      min_shop_purchases: "1",
      safe_neow: "1",
      limit: "25",
      ranked: "0",
    });
    await singleFlight("Finding SlayTheData runs", async () => {
      await refreshSlaythedataStatusQuietly();
      const result = await requestJson(`/api/slaythedata/candidates?${params.toString()}`);
      app.slaythedataCandidates = arrayOf(result.candidates);
      app.slaythedataSelectedRunId = app.slaythedataCandidates.length
        ? String(app.slaythedataCandidates[0].id)
        : "";
      applySelectedSlaythedataRunToStartControls();
      app.slaythedataLastError = null;
      renderCollectorPicker();
    });
  }

  async function loadSelectedSlaythedataRun() {
    const runId = app.slaythedataSelectedRunId || el.slaythedataCandidateSelect.value;
    if (!runId) {
      showError("Find and select a SlayTheData run first.");
      return;
    }
    await singleFlight("Loading SlayTheData run", async () => {
      pauseCollectorAutoRun();
      app.collector = await requestJson("/api/collector/start", {
        method: "POST",
        body: { run_id: Number(runId) },
      });
      applyGuidedRunStartControls(app.collector && app.collector.config);
      app.collectorLastError = null;
      app.slaythedataLastError = null;
    });
  }

  async function startGuidedLiveRun(options = {}) {
    if (!app.collector || !app.collector.active) {
      showError("Load or start a guided collector before starting the live run.");
      return;
    }
    await singleFlight("Starting guided live run", async () => {
      pauseCollectorAutoRun();
      try {
        const result = await requestJson("/api/collector/start-live-run", { method: "POST", body: {} });
        app.collector = result.collector || app.collector;
        app.collectorLastError = null;
        await refreshBridgeQuietly();
        await refreshCollectorReportQuietly();
        if (options.armAuto) {
          app.collectorAutoRun = true;
          scheduleCollectorAutoStep(850);
        }
      } catch (error) {
        app.collectorLastError = readableError(error);
        throw error;
      }
      renderCollector();
    });
  }

  function selectedSlaythedataCandidate() {
    const runId = app.slaythedataSelectedRunId || (el.slaythedataCandidateSelect && el.slaythedataCandidateSelect.value);
    if (!runId) return null;
    return app.slaythedataCandidates.find((candidate) => String(candidate.id) === String(runId)) || null;
  }

  function applySelectedSlaythedataRunToStartControls() {
    const candidate = selectedSlaythedataCandidate();
    if (!candidate) return;
    applyGuidedRunStartControls({
      character: el.startCharacterSelect && el.startCharacterSelect.value,
      ascension: el.startAscensionInput && el.startAscensionInput.value,
      seed_played: candidate.seed_played,
    });
  }

  function applyGuidedRunStartControls(config) {
    if (!config) return;
    const seed = firstDefined(config.seed_played, config.seed, null);
    if (seed !== null && seed !== undefined && el.startSeedInput) {
      el.startSeedInput.value = String(seed);
    }
    const ascension = firstDefined(config.ascension, config.ascension_level, null);
    if (ascension !== null && ascension !== undefined && el.startAscensionInput) {
      el.startAscensionInput.value = String(ascension);
    }
    const character = firstDefined(config.character, config.character_chosen, null);
    if (character && el.startCharacterSelect) {
      const normalized = String(character).trim().toUpperCase();
      const option = Array.from(el.startCharacterSelect.options).find((entry) => entry.value === normalized);
      if (option) el.startCharacterSelect.value = normalized;
    }
  }

  async function tickGuidedCollector(options = {}) {
    if (!app.collector || !app.collector.active) {
      showError("Start the guided collector before ticking it.");
      return;
    }
    await singleFlight(options.send ? "Sending guided choice" : "Previewing guided choice", async () => {
      try {
        app.collector = await requestJson("/api/collector/tick", {
          method: "POST",
          body: collectorTickPayload(options),
        });
        app.collectorLastError = null;
        await refreshBridgeQuietly();
        await refreshParityQuietly();
        await refreshCollectorReportQuietly();
        const suggestion = app.collector && app.collector.suggestion;
        if (suggestion && suggestion.status === "blocked") {
          throw new Error(firstDefined(suggestion.detail, suggestion.reason, "Collector blocked."));
        }
      } catch (error) {
        app.collectorLastError = readableError(error);
        throw error;
      }
    });
  }

  function collectorTickPayload(options = {}) {
    const body = { send: !!options.send };
    const maxDepth = Number.parseInt(el.maxDepthInput.value, 10);
    if (Number.isFinite(maxDepth)) body.max_depth = maxDepth;
    if (el.searchPolicySelect && el.searchPolicySelect.value) body.candidate = el.searchPolicySelect.value;
    const potions = usablePotions();
    if (potions.length) body.allowed_potions = allowedPotionsPayload();
    return body;
  }

  function startCollectorAutoRun() {
    if (!app.collector || !app.collector.active) {
      showError("Start the guided collector before auto-collecting.");
      return;
    }
    app.collectorAutoRun = true;
    app.collectorLastError = null;
    scheduleCollectorAutoStep(0);
    renderCollector();
  }

  function pauseCollectorAutoRun() {
    app.collectorAutoRun = false;
    if (app.collectorAutoTimer) {
      window.clearTimeout(app.collectorAutoTimer);
      app.collectorAutoTimer = null;
    }
    renderCollector();
  }

  function scheduleCollectorAutoStep(delayMs = 650) {
    if (!app.collectorAutoRun) return;
    if (app.collectorAutoTimer) window.clearTimeout(app.collectorAutoTimer);
    app.collectorAutoTimer = window.setTimeout(runCollectorAutoStep, delayMs);
  }

  async function runCollectorAutoStep() {
    app.collectorAutoTimer = null;
    if (!app.collectorAutoRun) return;
    if (app.inFlight) {
      scheduleCollectorAutoStep(350);
      return;
    }
    try {
      await refreshBridgeQuietly();
      await refreshCollectorQuietly();
      await refreshCollectorReportQuietly();
      if (!app.collector || !app.collector.active) {
        app.collectorAutoRun = false;
        renderCollector();
        return;
      }
      const waitReason = collectorAutoWaitReason();
      if (waitReason) {
        app.collectorLastError = null;
        renderCollector();
        scheduleCollectorAutoStep(850);
        return;
      }
      await tickGuidedCollector({ send: true });
      const blocker = app.collector && app.collector.blocker;
      if (app.lastError && !(blocker && isTransientCollectorBlocker(blocker))) {
        app.collectorAutoRun = false;
        renderCollector();
        return;
      }
      if (blocker && !isTransientCollectorBlocker(blocker)) {
        app.collectorAutoRun = false;
        renderCollector();
        return;
      }
      scheduleCollectorAutoStep(850);
    } catch (error) {
      const blocker = app.collector && app.collector.blocker;
      if (blocker && isTransientCollectorBlocker(blocker)) {
        app.collectorLastError = null;
        scheduleCollectorAutoStep(850);
      } else {
        app.collectorAutoRun = false;
        app.collectorLastError = readableError(error);
      }
      renderCollector();
    }
  }

  function collectorAutoWaitReason() {
    if (app.liveInvariantViolation) return "simulator/live mismatch needs acknowledgement";
    if (bridgeIdentityWarningText()) return "bridge client identity changed";
    const tcpReason = collectorTcpBlockerReason();
    if (tcpReason) return tcpReason;
    if (!app.bridge || !app.bridge.connected) return "bridge disconnected";
    if (app.bridge.exited) return "bridge exited";
    if (app.bridge.pending_command) return "waiting for pending bridge command";
    if (app.bridge.ready_for_command !== true) return "waiting for bridge ready state";
    return "";
  }

  function isTransientCollectorBlocker(blocker) {
    const reason = firstDefined(blocker && blocker.reason, "");
    return reason === "pending_command" || reason === "bridge_not_ready";
  }

  function collectorTcpBlockerReason() {
    const preflight = app.collector && app.collector.preflight;
    if (preflight && preflight.tcp_control_available === false) {
      return "fresh TCP bridge control is required";
    }
    return "";
  }

  async function stopGuidedCollector() {
    await singleFlight("Stopping guided collector", async () => {
      pauseCollectorAutoRun();
      app.collector = await requestJson("/api/collector/stop", { method: "POST", body: {} });
      app.collectorLastError = null;
      await refreshCollectorReportQuietly();
    });
  }

  async function refreshCollectorReport() {
    await singleFlight("Refreshing guided report", async () => {
      await refreshCollectorReportQuietly();
    });
  }

  async function refreshBridgeClients() {
    app.bridgeClientsLoading = true;
    app.bridgeClientsError = null;
    renderBridgeClients();
    try {
      const result = await requestJson("/api/bridge/clients");
      app.bridgeClients = arrayOf(result.clients);
    } catch (error) {
      app.bridgeClients = [];
      app.bridgeClientsError = readableError(error);
    } finally {
      app.bridgeClientsLoading = false;
      renderBridgeClients();
    }
  }

  async function refreshBridgeClientsQuietly() {
    try {
      const result = await requestJson("/api/bridge/clients");
      app.bridgeClients = arrayOf(result.clients);
      app.bridgeClientsError = null;
    } catch (error) {
      app.bridgeClients = [];
      app.bridgeClientsError = readableError(error);
    }
  }

  async function killBridgeClient(pid) {
    if (!pid) return;
    const client = app.bridgeClients.find((entry) => String(entry.pid) === String(pid));
    const label = client ? bridgeClientLabel(client) : `pid ${pid}`;
    const ok = window.confirm(`Kill bridge client ${label}?`);
    if (!ok) return;
    await singleFlight(`Killing bridge client ${pid}`, async () => {
      await requestJson("/api/bridge/clients/kill", {
        method: "POST",
        body: { pid },
      });
      await refreshBridgeClientsQuietly();
      await refreshBridgeQuietly();
      await refreshParityQuietly();
    });
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
    await submitBridgeCommand("state");
  }

  async function clearOrphanCommandMetadata() {
    if (!hasOrphanCommandMetadataProblem()) return;
    await singleFlight("Clearing stale command metadata", async () => {
      await requestJson("/api/bridge/orphan-command-metadata/clear", { method: "POST", body: {} });
      await refreshBridgeQuietly();
      await refreshCollectorQuietly();
    });
  }

  function hasOrphanCommandMetadataProblem() {
    const problems = arrayOf(app.collector && app.collector.preflight && app.collector.preflight.problems);
    return problems.some((problem) => String(problem).includes("next_command.json exists without next_command.txt"));
  }

  async function submitBridgeCommand(command) {
    if (!command) return;
    await singleFlight(`Sending ${humanize(command)}`, async () => {
      const result = await requestJson("/api/bridge/command", {
        method: "POST",
        body: { command, source_state_id: bridgeStateId() || undefined },
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
    const blocked = bridgeActionDisabledReason(action);
    if (blocked) {
      showError(blocked);
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
        body: {
          descriptor,
          source_state_id: firstDefined(action.source_state_id, action.sourceStateId, bridgeStateId()) || undefined,
        },
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
    app.attachFidelity = firstDefined(payload && payload.attach_fidelity, payload && payload.attachFidelity, state && state.attach_fidelity, state && state.attachFidelity, app.attachFidelity);
    app.strictReplayBlocker = firstDefined(payload && payload.strict_replay_blocker, payload && payload.strictReplayBlocker, state && state.strict_replay_blocker, state && state.strictReplayBlocker, app.strictReplayBlocker);
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
    renderLive();
    renderBoard();
    renderHand();
    renderActions();
    renderAllowedPotions();
    renderCollector();
    renderCollectorReport();
    renderSearch();
    renderBridge();
    renderBridgeClients();
    renderTrace();
    renderDebug();
    renderInvariantModal();
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
    el.liveModeButton.classList.toggle("active", app.viewMode === "live");
    el.simModeButton.classList.toggle("active", app.viewMode === "sim");
    el.fixtureControls.classList.toggle("hidden", app.viewMode !== "sim");
    el.liveBand.classList.toggle("fixture-mode", app.viewMode === "sim");
    el.searchButton.classList.toggle("hidden", app.viewMode === "live");
    el.applyBestButton.classList.toggle("hidden", app.viewMode === "live");
    el.refreshBridgeButton.disabled = app.inFlight;
    el.liveRequestStateButton.disabled = app.inFlight || (app.bridge && app.bridge.pending_command);
    el.requestBridgeStateButton.disabled = app.inFlight || (app.bridge && app.bridge.pending_command);
    el.clearOrphanCommandMetaButton.disabled = app.inFlight || !hasOrphanCommandMetadataProblem();
    el.refreshBridgeClientsButton.disabled = app.bridgeClientsLoading;
    const startReason = startLiveRunBlockedReason();
    const searchReason = liveSearchBlockedReason();
    const sendReason = liveSendBlockedReason();
    el.startLiveRunButton.disabled = app.inFlight || !!startReason;
    el.attachLiveButton.disabled = app.inFlight || !canAttachLiveSession();
    el.liveSearchButton.disabled = app.inFlight || !!searchReason;
    el.sendBestButton.disabled = app.inFlight || !!sendReason;
    el.sendBestReviewButton.disabled = app.inFlight || !!sendReason;
    el.startLiveRunButton.title = startReason || "Start a live run through CommunicationMod.";
    el.attachLiveButton.title = canAttachLiveSession() ? "Attach the latest observed live state to the simulator." : "No current observed bridge state is available to attach.";
    el.liveSearchButton.title = searchReason || "Search the attached live combat state.";
    el.sendBestButton.title = sendReason || "Send only the current recommended move.";
    el.sendBestReviewButton.title = sendReason || "Keep sending the current principal variation while simulator predictions match.";
    el.requestBridgeStateButton.title = "Trace-mutating: sends a CommunicationMod state command and records it.";
    el.clearOrphanCommandMetaButton.title = hasOrphanCommandMetadataProblem()
      ? "Remove orphan next_command.json while no bridge command is pending."
      : "Available only when preflight reports orphan command metadata.";
    el.liveRequestStateButton.title = "Trace-mutating: asks CommunicationMod to publish a fresh state.";
    el.refreshBridgeButton.title = "Read-only: refreshes local bridge files without sending a game command.";
    el.restoreSnapshotButton.disabled = !app.sessionId || app.inFlight || !hasLoadedSnapshotJson();
    renderCollector();

    el.stateSummary.textContent = summarizeState();
    el.lifecycleBadge.textContent = lifecycleText();
    el.lifecycleBadge.className = `status-badge ${lifecycleClass()}`;

    const bridgeWarning = bridgeIdentityWarningText();
    if (bridgeWarning || app.lastError) {
      el.actionError.textContent = bridgeWarning || app.lastError;
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
    const live = app.viewMode === "live";
    const actions = live
      ? orderedLiveActions(arrayOf(app.bridge && app.bridge.bridge_actions))
      : app.actions.length ? app.actions : app.lastError ? app.lastActions : [];
    el.actionsTitle.textContent = live ? "Manual Live Actions" : "Simulator Actions";
    const enabledCount = live
      ? actions.filter((action) => !bridgeActionDisabledReason(action) && action.enabled !== false).length
      : actions.filter((action) => action.enabled !== false).length;
    el.actionCount.textContent = live
      ? `${enabledCount} ready / ${actions.length} shown`
      : `${enabledCount} available`;
    clear(el.actionsPanel);
    if (!actions.length) {
      const reason = live ? emptyLiveActionReason() : emptyActionReason();
      empty(el.actionsPanel, reason);
      return;
    }

    el.actionsPanel.className = "button-grid";
    actions.forEach((action) => {
      const button = document.createElement("button");
      const disabledReason = live ? bridgeActionDisabledReason(action) : action.disabled_reason || action.disabledReason;
      const pendingReason = app.inFlight ? "A command is already in flight." : "";
      button.type = "button";
      button.className = "action-button";
      if (live && isRecommendedBridgeAction(action)) {
        button.classList.add("recommended-action");
      }
      if (live && disabledReason) {
        button.classList.add("unavailable-action");
      }
      button.disabled = app.inFlight || !!disabledReason || action.enabled === false;
      button.textContent = live ? bridgeActionLabel(action) : actionLabel(action);
      button.title = pendingReason || disabledReason || (live ? `Sends ${bridgeActionLabel(action)}` : sourceTitle(action));
      button.addEventListener("click", () => live ? submitBridgeAction(action) : submitAction(action));
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
    if (el.sendBestReviewButton) {
      el.sendBestReviewButton.classList.toggle("hidden", app.viewMode !== "live");
    }
    el.searchStatus.textContent = !combatSearch
      ? "Combat only"
      : app.liveAutoPlayPlan ? "Auto-playing"
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
        ["Best", best ? recommendationBestLabel() : "None"],
        ["HP loss", hpLossText(app.search)],
        ["Final HP", hpFinalText(app.search)],
        ["Value", firstDefined(app.search.value, "-")],
        ["Visits", firstDefined(app.search.visits, "-")],
        ["Win", percentText(app.search.win_probability)],
        ["Policy", searchPolicyLabel(app.search.config)],
      ]),
    );

    const pv = arrayOf(app.search.principal_variation);
    if (pv.length) {
      const list = document.createElement("ol");
      list.className = "pv-list";
      const activeIndex = Number.isInteger(app.search.planIndex) ? app.search.planIndex : 0;
      pv.forEach((item, index) => {
        const li = document.createElement("li");
        li.textContent = actionLabel(item);
        if (index < activeIndex) li.className = "pv-consumed";
        if (index === activeIndex) li.className = "pv-current";
        list.appendChild(li);
      });
      el.searchResult.appendChild(list);
    }
  }

  function renderAllowedPotions() {
    if (!el.allowedPotionsPanel) return;
    const previous = new Map(
      Array.from(el.allowedPotionsPanel.querySelectorAll("input[type='checkbox']")).map((input) => [
        potionToggleKey(input.dataset.potionName, input.dataset.potionIndex),
        input.checked,
      ]),
    );
    clear(el.allowedPotionsPanel);
    const potions = usablePotions();
    if (!potions.length) {
      el.allowedPotionsPanel.className = "potion-toggle-row empty";
      empty(el.allowedPotionsPanel, "No usable potions.");
      return;
    }
    el.allowedPotionsPanel.className = "potion-toggle-row";
    potions.forEach((potion) => {
      const label = document.createElement("label");
      label.className = "potion-toggle";
      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = previous.get(potionToggleKey(potion.name, potion.index)) ?? true;
      input.dataset.potionName = potion.name;
      input.dataset.potionIndex = potion.index === null || potion.index === undefined ? "" : String(potion.index);
      input.disabled = app.inFlight;
      const text = document.createElement("span");
      text.textContent = potion.name;
      label.append(input, text);
      el.allowedPotionsPanel.appendChild(label);
    });
  }

  function renderCollector() {
    if (!el.collectorStatusPanel) return;
    const active = app.collector && app.collector.active;
    const status = app.collector && app.collector.status || "idle";
    const suggestion = app.collector && app.collector.last_suggestion || app.collector && app.collector.suggestion;
    const preflight = app.collector && app.collector.preflight;
    const preflightProblems = arrayOf(preflight && preflight.problems);
    const tcpBlocker = collectorTcpBlockerReason();
    const guidedStartBlocker = !active
      ? "Load a guided script first."
      : app.inFlight
        ? "Another operation is running."
        : bridgeIdentityWarningText() || tcpBlocker || "";
    const canTick = active && !app.inFlight && !app.liveInvariantViolation && !bridgeIdentityWarningText() && !tcpBlocker && preflightProblems.length === 0;
    renderCollectorPicker();
    el.startCollectorButton.disabled = app.inFlight;
    el.findSlaythedataRunsButton.disabled = app.inFlight;
    el.loadSlaythedataRunButton.disabled = app.inFlight || !app.slaythedataSelectedRunId;
    el.startGuidedLiveRunButton.disabled = !!guidedStartBlocker;
    el.startGuidedAutoRunButton.disabled = !!guidedStartBlocker || app.collectorAutoRun;
    el.previewCollectorButton.disabled = !canTick;
    el.sendCollectorButton.disabled = !canTick || !app.bridge || app.bridge.pending_command;
    el.autoCollectorButton.disabled = !canTick || app.collectorAutoRun;
    el.pauseCollectorButton.disabled = !app.collectorAutoRun;
    el.stopCollectorButton.disabled = !active || app.inFlight;
    el.startGuidedLiveRunButton.title = guidedStartBlocker || "Send START from the loaded guided script seed.";
    el.startGuidedAutoRunButton.title = guidedStartBlocker || "Send START, then keep collecting until blocked.";
    el.previewCollectorButton.title = canTick ? "Preview the next guided decision without sending." : "Start a collector and keep the bridge ready.";
    el.sendCollectorButton.title = canTick ? "Send one safe guided action." : "Collector cannot send right now.";
    el.autoCollectorButton.title = canTick ? "Keep sending safe guided actions until blocked." : "Collector cannot auto-run right now.";
    el.pauseCollectorButton.title = app.collectorAutoRun ? "Pause guided auto-collection." : "Auto-collection is not running.";

    clear(el.collectorStatusPanel);
    if (app.collectorLastError) {
      el.collectorStatusPanel.className = "collector-status";
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = app.collectorLastError;
      el.collectorStatusPanel.appendChild(msg);
      return;
    }
    if (app.slaythedataLastError) {
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = app.slaythedataLastError;
      el.collectorStatusPanel.appendChild(msg);
    }
    renderSlaythedataStatus();
    renderCollectorPreflight(preflight);
    if (!active) {
      if (!el.collectorStatusPanel.childNodes.length) {
        empty(el.collectorStatusPanel, "No guided script loaded.");
      }
      return;
    }
    el.collectorStatusPanel.className = "collector-status";
    el.collectorStatusPanel.append(
      statBlock("Collector", [
        ["Status", status],
        ["Run", firstDefined(app.collector.source && app.collector.source.run_id, "-")],
        ["Seed", firstDefined(app.collector.config && app.collector.config.seed_played, "-")],
        ["Auto", app.collectorAutoRun ? collectorAutoWaitReason() || "Running" : "Paused"],
        ["History", firstDefined(app.collector.history_count, 0)],
      ]),
    );
    if (suggestion) {
      el.collectorStatusPanel.append(
        statBlock("Next", [
          ["Result", firstDefined(suggestion.status, "-")],
          ["Floor", firstDefined(suggestion.floor, "-")],
          ["Kind", firstDefined(suggestion.category, suggestion.mode, "-")],
          ["Target", firstDefined(suggestion.target, suggestion.detail, suggestion.reason, "-")],
          [
            "Command",
            firstDefined(
              suggestion.command,
              suggestion.combat_send && suggestion.combat_send.send_result && suggestion.combat_send.send_result.command,
              suggestion.descriptor && stringify(suggestion.descriptor),
              "-",
            ),
          ],
        ]),
      );
    }
    if (app.collector.blocker) {
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = firstDefined(app.collector.blocker.detail, app.collector.blocker.reason, "Collector blocked.");
      el.collectorStatusPanel.appendChild(msg);
    }
  }

  function renderCollectorPreflight(preflight) {
    if (!preflight) return;
    const problems = arrayOf(preflight.problems);
    const warnings = arrayOf(preflight.warnings);
    if (!problems.length && !warnings.length) return;
    const wrap = document.createElement("div");
    wrap.className = "collector-preflight";
    if (problems.length) {
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = `Preflight: ${problems.join("; ")}`;
      wrap.appendChild(msg);
    }
    if (warnings.length) {
      const msg = document.createElement("div");
      msg.className = "message info";
      msg.textContent = `Preflight warning: ${warnings.join("; ")}`;
      wrap.appendChild(msg);
    }
    el.collectorStatusPanel.appendChild(wrap);
  }

  function renderCollectorReport() {
    if (!el.collectorReportPanel) return;
    const report = app.collectorReport;
    clear(el.collectorReportPanel);
    el.collectorReportPanel.className = "collector-status";
    if (!report || report.missing) {
      el.collectorReportPanel.classList.add("empty");
      empty(el.collectorReportPanel, report && report.error ? report.error : "No guided collection report.");
      return;
    }

    const blocker = report.blocker || {};
    const selection = report.selection || {};
    const traceValidation = report.trace_validation || {};
    const traceName = report.trace_path ? String(report.trace_path).split(/[\\/]/).pop() : "-";
    el.collectorReportPanel.append(
      statBlock("Last Guided Report", [
        ["Result", report.ok ? "OK" : "Blocked"],
        ["Stop", firstDefined(report.stop_reason, "-")],
        ["Trace replay", validationText(traceValidation)],
        ["Protocol", protocolTraceText(traceValidation)],
        ["Producer", firstDefined(report.producer, "-")],
        ["Generated", dateText(report.generated_at)],
        ["Run", firstDefined(report.run_id, "-")],
        ["Selection", selectionText(selection)],
        ["Actions", firstDefined(report.actions_sent, 0)],
        ["Bridge", firstDefined(report.bridge_step, "-")],
        ["Trace", traceName],
      ]),
    );

    const problems = arrayOf(blocker.problems);
    const warnings = arrayOf(blocker.warnings);
    if (!report.ok) {
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = firstDefined(
        blocker.detail,
        blocker.reason,
        problems.length ? problems.join("; ") : null,
        "Guided collection did not complete.",
      );
      el.collectorReportPanel.appendChild(msg);
    } else if (warnings.length) {
      const msg = document.createElement("div");
      msg.className = "message info";
      msg.textContent = `Warnings: ${warnings.join("; ")}`;
      el.collectorReportPanel.appendChild(msg);
    }
    const skippedUnsupported = arrayOf(selection.skipped_unsupported);
    if (skippedUnsupported.length) {
      const msg = document.createElement("div");
      msg.className = "message info";
      msg.textContent = `Skipped: ${skippedUnsupported.map(skippedCandidateText).join("; ")}`;
      el.collectorReportPanel.appendChild(msg);
    }
    if (traceValidation && traceValidation.verified === false) {
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = `Trace replay not verified: ${firstDefined(
        traceValidation.blocker_detail,
        traceValidation.blocker_reason,
        traceValidation.reason,
        traceValidation.stop_reason,
        "unknown reason",
      )}`;
      el.collectorReportPanel.appendChild(msg);
    }

    const history = arrayOf(report.history_tail);
    if (history.length) {
      el.collectorReportPanel.append(
        statBlock("Recent Collector Events", history.map((entry) => [
          firstDefined(entry.event, entry.status, entry.suggestion_status, "event"),
          compactHistoryText(entry),
        ])),
      );
    }
  }

  function compactHistoryText(entry) {
    if (!entry || typeof entry !== "object") return entry;
    const blocker = entry.blocker && typeof entry.blocker === "object" ? entry.blocker : null;
    const parts = [
      firstDefined(entry.reason, blocker && blocker.reason, null),
      firstDefined(entry.detail, blocker && blocker.detail, null),
      firstDefined(entry.category, entry.floor === undefined ? null : `floor ${entry.floor}`, null),
      firstDefined(entry.command, null),
    ].filter((part) => part !== null && part !== undefined && part !== "");
    return parts.length ? parts.join(" / ") : entry;
  }

  function selectionText(selection) {
    if (!selection || !selection.mode) return "-";
    const mode = String(selection.mode);
    const considered = selection.considered_count || selection.candidate_count
      ? ` ${firstDefined(selection.considered_count, "-")}/${firstDefined(selection.candidate_count, "-")}`
      : "";
    const skipped = selection.skipped_unsupported_count ? `, skipped ${selection.skipped_unsupported_count}` : "";
    return `${mode}${considered}${skipped}`;
  }

  function skippedCandidateText(entry) {
    if (!entry || typeof entry !== "object") return stringify(entry);
    return `${firstDefined(entry.run_id, "-")}: ${firstDefined(entry.reason, entry.detail, "unsupported")}`;
  }

  function protocolTraceText(validation) {
    if (!validation) return "-";
    const accepts = firstDefined(validation.command_accepts, 0);
    const timeouts = firstDefined(validation.command_observed_timeouts, 0);
    return `${accepts} accepted / ${timeouts} timed out`;
  }

  function validationText(validation) {
    if (!validation) return "-";
    if (validation.verified === true) {
      const steps = validation.steps === undefined || validation.steps === null ? "" : `, ${validation.steps} steps`;
      return `verified${steps}`;
    }
    if (validation.verified === false) {
      return firstDefined(
        validation.reason,
        validation.blocker_reason,
        validation.stop_reason,
        "not verified",
      );
    }
    return "-";
  }

  function renderSlaythedataStatus() {
    const status = app.slaythedataStatus;
    if (!status) return;
    const problems = arrayOf(status.problems);
    const warnings = arrayOf(status.warnings);
    const msg = document.createElement("div");
    msg.className = problems.length ? "message error" : warnings.length ? "message info" : "message success";
    const available = firstDefined(status.exportable_candidate_available, null);
    const hasCounts = status.counts_included === true;
    const countText = hasCounts
      ? `${firstDefined(status.runs_count, "-")} runs, ${firstDefined(status.chunk_runs_count, "-")} export rows`
      : "fast readiness check";
    const prefix = problems.length ? "SlayTheData blocked" : warnings.length ? "SlayTheData usable with warnings" : "SlayTheData ready";
    const detail = problems.length
      ? problems.join("; ")
      : warnings.length
        ? warnings.join("; ")
        : available === true
          ? "supported guided candidates available"
          : available === false
            ? "no supported guided candidates for current filters"
            : "candidate availability unknown";
    msg.textContent = `${prefix}: ${detail} (${countText})`;
    el.collectorStatusPanel.appendChild(msg);
  }

  function renderCollectorPicker() {
    if (!el.slaythedataCandidateSelect) return;
    const selected = app.slaythedataSelectedRunId || el.slaythedataCandidateSelect.value || "";
    clear(el.slaythedataCandidateSelect);
    if (!app.slaythedataCandidates.length) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = "No SlayTheData runs loaded";
      el.slaythedataCandidateSelect.appendChild(option);
      app.slaythedataSelectedRunId = "";
      return;
    }
    for (const candidate of app.slaythedataCandidates) {
      const option = document.createElement("option");
      option.value = String(candidate.id);
      option.textContent = slaythedataCandidateLabel(candidate);
      el.slaythedataCandidateSelect.appendChild(option);
    }
    const hasSelected = app.slaythedataCandidates.some((candidate) => String(candidate.id) === String(selected));
    app.slaythedataSelectedRunId = hasSelected ? String(selected) : String(app.slaythedataCandidates[0].id);
    el.slaythedataCandidateSelect.value = app.slaythedataSelectedRunId;
  }

  function renderLive() {
    if (!el.liveStatusPanel) return;
    const lifecycle = bridgeLifecycle();
    const summary = (app.bridge && app.bridge.summary) || {};
    const phase = firstDefined(summary.room_phase, summary.screen_type, app.phase, "-");
    const startBlocker = startLiveRunBlockedReason();
    const searchBlocker = liveSearchBlockedReason();
    const sendBlocker = liveSendBlockedReason();
    const startStatus = summary.in_game === false ? startBlocker : null;

    el.liveStatusBadge.textContent = lifecycle.label;
    el.liveStatusBadge.className = `status-badge ${bridgeLifecycleClass(lifecycle.status)}`;
    el.liveSummary.textContent = app.bridge
      ? "Auto-refresh reads bridge files only."
      : "Waiting for bridge status.";
    renderLiveWorkflow({ searchBlocker, sendBlocker, startStatus });

    clear(el.liveStatusPanel);
    el.liveStatusPanel.append(
      statBlock("Bridge", [
        ["Connection", lifecycle.label],
        ["Client", firstDefined(app.bridge && app.bridge.client_pid, "-")],
        ["Step", firstDefined(app.bridge && app.bridge.last_state_step, "-")],
        ["Phase", phase],
        ["Ready", firstDefined(app.bridge && app.bridge.ready_for_command, "-")],
        ["Pending", app.bridge && app.bridge.pending_command ? "Yes" : "No"],
      ]),
      statBlock("Assistant", [
        ["Session", app.mode === "live_bridge" ? "Live state attached" : "Not attached"],
        ["Attached step", firstDefined(app.liveBridgeStep, "-")],
        ["Fidelity", attachFidelityText()],
        ["Replay", strictReplayText()],
        ["Plan guard", livePlanGuardText()],
        ["Status", sendBlocker || searchBlocker || startStatus || "Ready"],
      ]),
    );

    el.liveReason.textContent = liveReasonText({ sendBlocker, searchBlocker, startStatus });
  }

  function renderLiveWorkflow({ searchBlocker, sendBlocker, startStatus }) {
    if (!el.liveWorkflow) return;
    clear(el.liveWorkflow);
    const attached = app.mode === "live_bridge" && app.liveBridgeStateId === bridgeStateId();
    const searched = Boolean(app.search && app.liveSearchBridgeStateId === bridgeStateId());
    const sendReady = !sendBlocker && searched;
    el.liveWorkflow.append(
      workflowStep("1", attached ? "Attached" : startStatus ? "Start run" : "Start / attach", attached ? "done" : startStatus ? "blocked" : "ready"),
      workflowStep("2", searched ? "Recommendation ready" : "Search best", searched ? "done" : searchBlocker ? "blocked" : "ready"),
      workflowStep("3", sendReady ? "Send ready" : "Send safely", sendReady ? "ready" : sendBlocker ? "blocked" : "ready"),
    );
  }

  function workflowStep(number, labelText, state) {
    const node = document.createElement("span");
    node.className = `workflow-step ${state || "ready"}`;
    const badge = document.createElement("strong");
    badge.textContent = number;
    const label = document.createElement("span");
    label.textContent = labelText;
    node.append(badge, label);
    return node;
  }

  function renderBridgeClients() {
    if (!el.bridgeClientsPanel) return;
    clear(el.bridgeClientsPanel);
    el.refreshBridgeClientsButton.disabled = app.bridgeClientsLoading;
    if (app.bridgeClientsLoading) {
      empty(el.bridgeClientsPanel, "Loading bridge clients.");
      return;
    }
    if (app.bridgeClientsError) {
      el.bridgeClientsPanel.className = "bridge-clients-panel";
      const msg = document.createElement("div");
      msg.className = "message error";
      msg.textContent = app.bridgeClientsError;
      el.bridgeClientsPanel.appendChild(msg);
      return;
    }
    if (!app.bridgeClients.length) {
      empty(el.bridgeClientsPanel, "No bridge client PIDs found in current status or recent traces.");
      return;
    }

    el.bridgeClientsPanel.className = "bridge-clients-panel";
    app.bridgeClients.forEach((client) => {
      const card = document.createElement("article");
      card.className = `bridge-client-card${client.current ? " current" : ""}${client.alive ? "" : " dead"}`;

      const main = document.createElement("div");
      main.className = "bridge-client-main";
      const title = document.createElement("strong");
      title.textContent = bridgeClientLabel(client);
      const meta = document.createElement("span");
      meta.textContent = bridgeClientMeta(client);
      main.append(title, meta);

      const chips = document.createElement("div");
      chips.className = "bridge-client-chips";
      chips.appendChild(clientChip(client.current ? "Current" : "Extra", client.current ? "good" : "warn"));
      chips.appendChild(clientChip(client.alive ? "Alive" : "Dead", client.alive ? "good" : "neutral"));
      arrayOf(client.sources).forEach((source) => chips.appendChild(clientChip(humanize(source), "neutral")));

      const button = document.createElement("button");
      button.type = "button";
      button.className = "danger-button";
      button.textContent = client.current ? "Kill current" : "Kill";
      button.disabled = app.inFlight || !client.killable;
      button.title = client.killable ? "Terminate this bridge client process." : "This client is not alive or cannot be killed safely.";
      button.addEventListener("click", () => killBridgeClient(client.pid));

      card.append(main, chips, button);
      el.bridgeClientsPanel.appendChild(card);
    });
  }

  function bridgeClientLabel(client) {
    const name = firstDefined(client.name, "process");
    return `${name} pid ${firstDefined(client.pid, "?")}`;
  }

  function bridgeClientMeta(client) {
    const trace = arrayOf(client.trace_paths)[0];
    const traceName = trace ? trace.split(/[\\/]/).pop() : "no trace";
    const age = typeof client.trace_age_seconds === "number" ? `${Math.round(client.trace_age_seconds)}s old` : "unknown age";
    const started = client.started_at ? `started ${client.started_at}` : "start unknown";
    return `${traceName} / ${age} / ${started}`;
  }

  function clientChip(text, kind) {
    const chip = document.createElement("span");
    chip.className = `client-chip ${kind || "neutral"}`;
    chip.textContent = text;
    return chip;
  }

  function renderBridge() {
    clear(el.bridgePanel);
    if (!app.bridge) {
      empty(el.bridgePanel, "Bridge status not loaded.");
      return;
    }
    el.bridgePanel.className = "bridge-panel";
    const lifecycle = bridgeLifecycle();
    el.bridgePanel.appendChild(bridgeLifecycleSummary(lifecycle));
    el.bridgePanel.append(
      statBlock("CommunicationMod", [
        ["State", lifecycle.label],
        ["Step", firstDefined(app.bridge.last_state_step, "-")],
        ["Ready", firstDefined(app.bridge.ready_for_command, "-")],
        ["Pending command", app.bridge.pending_command ? "Yes" : "No"],
        ["Last command", firstDefined(app.bridge.last_command, "-")],
        ["Command sent", firstDefined(app.bridge.command_sent_at, "-")],
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

  function bridgeLifecycle() {
    if (!app.bridge) {
      return {
        status: "unknown",
        label: "Unknown",
        detail: "",
      };
    }
    const lifecycle = app.bridge && app.bridge.bridge_lifecycle;
    if (lifecycle && typeof lifecycle === "object") {
      const status = firstDefined(lifecycle.status, "unknown");
      return {
        status,
        label: firstDefined(lifecycle.label, humanize(status)),
        detail: firstDefined(lifecycle.detail, ""),
      };
    }
    const status = app.bridge.connected
      ? app.bridge.stale
        ? "stale"
        : "ready"
      : app.bridge.exited
        ? "exited"
        : "disconnected";
    return {
      status,
      label: humanize(status),
      detail: "",
    };
  }

  function bridgeLifecycleSummary(lifecycle) {
    const wrapper = document.createElement("div");
    wrapper.className = "bridge-lifecycle";

    const badge = document.createElement("span");
    badge.className = `status-badge bridge-lifecycle-badge ${bridgeLifecycleClass(lifecycle.status)}`;
    badge.textContent = lifecycle.label;
    wrapper.appendChild(badge);

    if (lifecycle.detail) {
      const detail = document.createElement("span");
      detail.className = "bridge-lifecycle-detail";
      detail.textContent = lifecycle.detail;
      wrapper.appendChild(detail);
    }

    return wrapper;
  }

  function bridgeLifecycleClass(status) {
    switch (status) {
      case "ready":
        return "good";
      case "waiting_for_command_ack":
      case "waiting_for_next_state":
      case "waiting_for_observed_state":
        return "busy";
      case "disconnected":
      case "exited":
      case "stale":
        return "bad";
      default:
        return "neutral";
    }
  }

  function bridgeActionsSection() {
    const wrapper = document.createElement("div");
    wrapper.className = "bridge-action-section";
    wrapper.appendChild(line("h3", "Bridge Actions"));

    const actions = orderedLiveActions(arrayOf(app.bridge && app.bridge.bridge_actions));
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
      const disabledReason = bridgeActionDisabledReason(action);
      const sourceStateId = firstDefined(action.source_state_id, action.sourceStateId, null);
      button.type = "button";
      button.className = "bridge-action-button";
      button.disabled = app.inFlight || !!disabledReason || action.enabled === false;
      button.textContent = bridgeActionLabel(action);
      button.title = disabledReason || (app.bridge.pending_command ? "Bridge command pending." : `Derived from bridge state ${stringify(sourceStateId || bridgeStateId() || "-")}`);
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

  function orderedLiveActions(actions) {
    return actions.slice().sort((left, right) => liveActionSortRank(left) - liveActionSortRank(right));
  }

  function liveActionSortRank(action) {
    const descriptor = action && action.descriptor || {};
    const actionId = String(firstDefined(action && action.action_id, action && action.actionId, ""));
    if (descriptor.kind === "DiscardPotionSlot" || actionId.startsWith("discard-potion")) return 90;
    return 0;
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
      predicted_hp_loss: firstDefined(recommendation.predicted_hp_loss, recommendation.predictedHpLoss, null),
      predicted_final_hp: firstDefined(recommendation.predicted_final_hp, recommendation.predictedFinalHp, null),
      config: recommendation.config,
      planIndex: 0,
    };
  }

  function predictionPayload(action) {
    const descriptor = action.descriptor || (action.kind ? action : null);
    return {
      action_id: firstDefined(action.action_id, action.actionId, action.id, undefined),
      descriptor: descriptor || undefined,
      exact_action_json: firstDefined(action.exact_action_json, action.exactActionJson, undefined),
      source_state_id: currentStateId(),
    };
  }

  function nextPrincipalVariationIndex() {
    if (!app.search) return null;
    const current = Number.isInteger(app.search.planIndex) ? app.search.planIndex : 0;
    return current + 1;
  }

  function verifyPendingLivePrediction(observedStateId) {
    const pending = app.livePendingPrediction;
    if (!pending) return false;
    app.livePendingPrediction = null;
    const expected = firstDefined(pending.predicted_state_id, pending.predictedStateId, null);
    if (expected && observedStateId && expected === observedStateId) return true;

    const violation = {
      title: "Simulator prediction mismatch",
      message: "The live game reached a different state than the simulator predicted. Cached recommendations are blocked until you inspect and reattach/research.",
      expected,
      observed: observedStateId || "-",
      source: firstDefined(pending.source_state_id, "-"),
      bridgeState: firstDefined(pending.bridge_state_id, "-"),
      bridgeStep: firstDefined(pending.bridge_step, "-"),
      action: firstDefined(pending.action_label, "Unknown action"),
    };
    app.liveInvariantViolation = violation;
    app.search = null;
    app.liveSearchBridgeStateId = null;
    app.liveSendAction = null;
    app.livePendingPlanIndex = null;
    app.liveAutoPlayPlan = false;
    window.alert(`${violation.title}\n\nExpected: ${violation.expected}\nObserved: ${violation.observed}\nAction: ${violation.action}`);
    return false;
  }

  function advanceLiveSearchPlan(previousSearch, nextIndex, bridgeId) {
    const pv = arrayOf(previousSearch && previousSearch.principal_variation);
    if (!Number.isInteger(nextIndex) || nextIndex < 0 || nextIndex >= pv.length) {
      app.search = null;
      app.liveAutoPlayPlan = false;
      return false;
    }
    app.search = Object.assign({}, previousSearch, {
      bestAction: pv[nextIndex],
      planIndex: nextIndex,
    });
    app.liveSearchBridgeStateId = bridgeId;
    app.liveSendAction = liveBridgeActionForBest();
    app.livePendingPlanIndex = null;
    return true;
  }

  function acknowledgeInvariantViolation() {
    app.liveInvariantViolation = null;
    render();
  }

  function renderInvariantModal() {
    if (!el.invariantModal) return;
    const violation = app.liveInvariantViolation;
    el.invariantModal.classList.toggle("hidden", !violation);
    if (!violation) return;
    el.invariantTitle.textContent = violation.title || "Live plan stopped";
    el.invariantMessage.textContent = violation.message || "The simulator and live game diverged.";
    clear(el.invariantFacts);
    el.invariantFacts.append(
      statBlock("Mismatch", [
        ["Expected", firstDefined(violation.expected, "-")],
        ["Observed", firstDefined(violation.observed, "-")],
        ["Source", firstDefined(violation.source, "-")],
        ["Bridge state", firstDefined(violation.bridgeState, "-")],
        ["Bridge step", firstDefined(violation.bridgeStep, "-")],
        ["Action", firstDefined(violation.action, "-")],
      ]),
    );
  }

  function canAttachLiveSession() {
    if (!app.bridge || !app.bridge.connected || app.bridge.pending_command) return false;
    const current = app.bridge.current_state || {};
    return !!observedBridgeGameState(current);
  }

  function startLiveRunBlockedReason() {
    if (app.bridgeIdentityWarning) return "Bridge client changed; reload after closing extra bridge clients.";
    if (!app.bridge) return "Bridge status not loaded.";
    if (app.bridge.exited) return "Bridge exited.";
    if (!app.bridge.connected) return "Bridge disconnected.";
    if (app.bridge.pending_command) return "Waiting for pending bridge command.";
    const commands = arrayOf(app.bridge.available_commands).map((command) => String(command).toLowerCase());
    const staleStartFromMenu = app.bridge.stale && commands.includes("start") && app.bridge.summary && app.bridge.summary.in_game === false;
    if (app.bridge.stale && !staleStartFromMenu) return "Bridge state is stale.";
    if (!commands.includes("start")) {
      return "Start is not available from the current game screen.";
    }
    return null;
  }

  function liveSearchBlockedReason() {
    if (app.liveInvariantViolation) return "Simulator/live mismatch needs acknowledgement.";
    if (app.bridgeIdentityWarning) return "Bridge client changed; reload after closing extra bridge clients.";
    if (app.viewMode !== "live") return "Switch to Live Game mode.";
    if (!app.bridge) return "Bridge status not loaded.";
    if (app.bridge.exited) return "Bridge exited.";
    if (!app.bridge.connected) return "Bridge disconnected.";
    if (app.bridge.pending_command) return "Waiting for pending bridge command.";
    if (app.bridge.summary && app.bridge.summary.in_game === false) return "Start a run to enable combat search.";
    if (!canAttachLiveSession()) return "No observed game state yet.";
    if (!bridgeLooksLikeCombat()) return "Search is available once the live game is in combat.";
    return null;
  }

  function liveSendBlockedReason() {
    if (app.liveInvariantViolation) return "Simulator/live mismatch needs acknowledgement.";
    const searchBlocker = liveSearchBlockedReason();
    if (searchBlocker) return searchBlocker;
    if (!app.search || !app.search.bestAction) return "Run search first.";
    if (app.liveSearchBridgeStateId !== bridgeStateId()) return "Recommendation is for an older bridge state.";
    if (!liveBridgeActionForBest()) return "Recommendation cannot be mapped to a current bridge command.";
    return null;
  }

  function bridgeActionDisabledReason(action) {
    if (app.liveInvariantViolation) return "Simulator/live mismatch needs acknowledgement.";
    if (app.bridgeIdentityWarning) return "Bridge client changed.";
    if (!app.bridge) return "Bridge status not loaded.";
    if (app.bridge.exited) return "Bridge exited.";
    if (!app.bridge.connected) return "Bridge disconnected.";
    if (app.bridge.pending_command) return "Waiting for pending bridge command.";
    const sourceStateId = firstDefined(action && action.source_state_id, action && action.sourceStateId, null);
    if (app.bridge.stale && (!sourceStateId || sourceStateId !== bridgeStateId())) {
      return "Bridge state is stale.";
    }
    if (action && (action.disabled_reason || action.disabledReason)) {
      return action.disabled_reason || action.disabledReason;
    }
    return null;
  }

  function bridgeLooksLikeCombat() {
    const summary = (app.bridge && app.bridge.summary) || {};
    if (summary.combat) return true;
    if (String(firstDefined(summary.room_phase, "")).toUpperCase() === "COMBAT") return true;
    const current = app.bridge && app.bridge.current_state;
    const observed = observedBridgeGameState(current);
    return !!(observed && observed.combat_state);
  }

  function observedBridgeGameState(current) {
    if (!current) return null;
    const message = current.message;
    if (message && typeof message === "object") {
      if (message.game_state && typeof message.game_state === "object") return message.game_state;
      if (looksLikeCommunicationModState(message)) return message;
    }
    if (current.game_state && typeof current.game_state === "object") return current.game_state;
    if (looksLikeCommunicationModState(current)) return current;
    return null;
  }

  function looksLikeCommunicationModState(value) {
    if (!value || typeof value !== "object") return false;
    return [
      "combat_state",
      "screen_type",
      "choice_list",
      "current_hp",
      "player_hp",
      "floor",
      "deck",
      "relics",
    ].some((key) => Object.prototype.hasOwnProperty.call(value, key));
  }

  function emptyLiveActionReason() {
    if (!app.bridge) return "Bridge status not loaded.";
    if (app.bridge.exited) return "Bridge exited.";
    if (!app.bridge.connected) return "Bridge disconnected.";
    if (app.bridge.pending_command) return "Waiting for pending bridge command.";
    if (app.bridge.stale) return "Bridge state is stale. Use Manual State only if you intentionally want a trace-recorded state command.";
    return "No live actions available on this screen.";
  }

  function liveBridgeActionForBest() {
    if (!app.bridge || !app.search || !app.search.bestAction) return null;
    if (app.liveSearchBridgeStateId !== bridgeStateId()) return null;
    const best = app.search.bestAction;
    const bridgeActions = arrayOf(app.bridge.bridge_actions);
    if (!bridgeActions.length) return null;
    if (isEndTurnAction(best)) {
      return bridgeActions.find((action) => String(action.command || "").toUpperCase() === "END") || null;
    }
    const play = exactRunPlayCard(best);
    if (!play) return null;
    const slot = observedHandSlotForCardId(play.card_id);
    if (slot === null) return null;
    const targetSlot = observedMonsterSlotForTarget(play.target);
    return bridgeActions.find((action) => {
      const descriptor = action.descriptor || {};
      if (descriptor.kind !== "PlayHandSlot") return false;
      if (Number(descriptor.hand_slot) !== Number(slot)) return false;
      if (play.target === null || play.target === undefined) return descriptor.target_slot === undefined || descriptor.target_slot === null;
      return Number(descriptor.target_slot) === Number(targetSlot);
    }) || null;
  }

  function isRecommendedBridgeAction(action) {
    const recommended = liveBridgeActionForBest();
    if (!recommended || !action) return false;
    return bridgeActionKey(recommended) === bridgeActionKey(action);
  }

  function bridgeActionKey(action) {
    return JSON.stringify({
      command: firstDefined(action && action.command, ""),
      descriptor: firstDefined(action && action.descriptor, null),
      source: firstDefined(action && action.source_state_id, action && action.sourceStateId, null),
    });
  }

  function bridgeIdentity(bridge) {
    if (!bridge || !bridge.connected) return null;
    const clientPid = firstDefined(bridge.client_pid, bridge.clientPid, null);
    const tracePath = firstDefined(bridge.trace_path, bridge.tracePath, null);
    if (clientPid === null && tracePath === null) return null;
    return {
      clientPid: clientPid === null ? null : String(clientPid),
      tracePath: tracePath === null ? null : String(tracePath),
    };
  }

  function bridgeIdentityWarningText() {
    const warning = app.bridgeIdentityWarning;
    if (!warning) return "";
    return `Bridge client changed from ${identityText(warning.previous)} to ${identityText(warning.current)}. Close extra bridge clients and reload before sending more live commands.`;
  }

  function identityText(identity) {
    if (!identity) return "unknown";
    const trace = identity.tracePath ? identity.tracePath.split(/[\\/]/).pop() : "no trace";
    return `pid ${firstDefined(identity.clientPid, "?")} / ${trace}`;
  }

  function recommendationBestLabel() {
    const liveAction = app.viewMode === "live" ? liveBridgeActionForBest() : null;
    if (liveAction) return bridgeActionLabel(liveAction);
    const best = app.search && app.search.bestAction;
    return best ? actionLabel(best) : "None";
  }

  function exactRunPlayCard(action) {
    if (!action) return null;
    if (action.kind === "PlayCard") {
      return { card_id: action.card_id, target: action.target };
    }
    const descriptor = action.descriptor || {};
    const descriptorPayload = descriptor.action && descriptor.action.PlayCard;
    if (descriptorPayload) {
      return { card_id: descriptorPayload.card_id, target: descriptorPayload.target };
    }
    const payload = action.action && action.action.PlayCard;
    if (payload) return { card_id: payload.card_id, target: payload.target };
    return null;
  }

  function isEndTurnAction(action) {
    if (!action) return false;
    if (action.kind === "EndTurn" || action.action_kind === "end_turn") return true;
    const descriptor = action.descriptor || {};
    if (descriptor.kind === "EndTurn" || descriptor.action_kind === "end_turn") return true;
    if (descriptor.action === "EndTurn" || action.action === "EndTurn") return true;
    return false;
  }

  function observedHandSlotForCardId(cardId) {
    const hand = arrayOf(app.bridge && app.bridge.summary && app.bridge.summary.combat && app.bridge.summary.combat.hand);
    const direct = hand.find((entry) => String(firstDefined(entry.id, entry.card_id, entry.cardId, "")) === String(cardId));
    if (direct) {
      const directSlot = firstDefined(direct.index, direct.slot, direct.hand_slot, direct.handSlot, null);
      if (directSlot !== null) return directSlot;
    }

    const simHand = arrayOf(app.state && app.state.combat && app.state.combat.piles && app.state.combat.piles.hand);
    const simIndex = simHand.findIndex((entry) => String(firstDefined(entry.id, entry.card_id, entry.cardId, "")) === String(cardId));
    const card = simIndex >= 0 ? hand[simIndex] : null;
    const slot = card && firstDefined(card.index, card.slot, card.hand_slot, card.handSlot, null);
    return slot === undefined ? null : slot;
  }

  function observedMonsterSlotForTarget(target) {
    if (target === null || target === undefined) return null;
    const monsters = arrayOf(app.bridge && app.bridge.summary && app.bridge.summary.combat && app.bridge.summary.combat.monsters);
    const direct = monsters.find((entry) => String(firstDefined(entry.id, entry.monster_id, entry.monsterId, entry.index, "")) === String(target));
    if (direct) {
      const directSlot = firstDefined(direct.index, direct.slot, direct.target_slot, direct.targetSlot, null);
      if (directSlot !== null) return directSlot;
    }

    const simMonsters = arrayOf(app.state && app.state.combat && app.state.combat.monsters);
    const simIndex = simMonsters.findIndex((entry) => String(firstDefined(entry.id, entry.monster_id, entry.monsterId, "")) === String(target));
    const monster = simIndex >= 0 ? monsters[simIndex] : null;
    const slot = monster && firstDefined(monster.index, monster.slot, monster.target_slot, monster.targetSlot, null);
    return slot === undefined ? null : slot;
  }

  function searchPolicyLabel(config) {
    if (!config) return "-";
    const algorithm = firstDefined(config.algorithm, "-");
    const objective = firstDefined(config.objective, "-");
    const depth = firstDefined(config.max_depth, config.maxDepth, "-");
    const width = firstDefined(config.beam_width, config.beamWidth, "-");
    return `${algorithm} / ${objective} / d${depth} / w${width}`;
  }

  function slaythedataCandidateLabel(candidate) {
    const runId = firstDefined(candidate && candidate.id, "?");
    const seed = firstDefined(candidate && candidate.seed_played, "no-seed");
    const floor = firstDefined(candidate && candidate.floor_reached, "?");
    const result = candidate && candidate.victory ? "win" : "loss";
    const path = firstDefined(candidate && candidate.path_length, "?");
    const cards = firstDefined(candidate && candidate.card_choice_count, 0);
    const events = firstDefined(candidate && candidate.event_choice_count, 0);
    const shops = firstDefined(candidate && candidate.shop_purchase_count, 0);
    const potions = firstDefined(candidate && candidate.potion_usage_count, 0);
    const score = firstDefined(candidate && candidate.guided_score, 0);
    const neowBonus = firstDefined(candidate && candidate.neow_bonus, candidate && candidate.neowBonus, "");
    const neowCost = firstDefined(candidate && candidate.neow_cost, candidate && candidate.neowCost, "");
    const neow = neowBonus ? ` Neow ${neowBonus}${neowCost ? `:${neowCost}` : ""} |` : "";
    return `#${runId} ${seed} F${floor} path ${path} ${result} |${neow} score ${score} cards ${cards} events ${events} shops ${shops} pots ${potions}`;
  }

  function attachFidelityText() {
    const mode = firstDefined(app.attachFidelity, null);
    if (!mode) return app.mode === "live_bridge" ? "Observed state" : "-";
    if (mode === "observed_state") return "Observed state";
    if (mode === "seed_replay") return "Seed replay";
    return humanize(mode);
  }

  function strictReplayText() {
    if (app.attachFidelity === "seed_replay") return "Verified";
    const blocker = app.strictReplayBlocker;
    if (!blocker) return app.mode === "live_bridge" ? "Not used" : "-";
    const reason = firstDefined(blocker.stop_reason, blocker.blocker && blocker.blocker.category, "Fallback");
    if (reason === "missing_start") return "No START in trace";
    return humanize(reason);
  }

  function livePlanGuardText() {
    if (app.liveInvariantViolation) return "Stopped";
    if (app.livePendingPrediction) return "Awaiting observed state";
    if (app.search && app.liveSearchBridgeStateId === bridgeStateId()) return "Predicted";
    return "Idle";
  }

  function liveReasonText({ sendBlocker, searchBlocker, startStatus }) {
    if (sendBlocker) return `Blocked: ${sendBlocker}`;
    if (searchBlocker) return `Blocked: ${searchBlocker}`;
    if (startStatus) return `Setup: ${startStatus}`;
    if (app.search && app.search.bestAction) return `Will send: ${recommendationBestLabel()}`;
    return "Ready: attach or search when combat is available.";
  }

  function allowedPotionsPayload() {
    const potions = usablePotions();
    if (!potions.length) return [];
    const checked = Array.from(el.allowedPotionsPanel.querySelectorAll("input[type='checkbox']:checked"))
      .map((input) => input.dataset.potionName)
      .filter(Boolean);
    return checked;
  }

  function potionToggleKey(name, index) {
    return `${firstDefined(index, "")}:${name}`;
  }

  function usablePotions() {
    const bridgePotions = arrayOf(app.bridge && app.bridge.summary && app.bridge.summary.potions)
      .filter((potion) => potion && potion.can_use)
      .map((potion) => ({
        name: String(firstDefined(potion.name, potion.id, `Potion ${firstDefined(potion.index, "?")}`)),
        index: firstDefined(potion.index, null),
      }));
    if (bridgePotions.length) return bridgePotions;

    return arrayOf(app.state && app.state.potions)
      .map((potion, index) => ({
        name: typeof potion === "string" ? potion : String(firstDefined(potion.name, potion.id, `Potion ${index + 1}`)),
        index,
      }))
      .filter((potion) => potion.name && potion.name !== "Potion Slot");
  }

  function hpLossText(search) {
    const loss = firstDefined(search && search.predicted_hp_loss, null);
    if (loss === null || loss === undefined) return "-";
    const rounded = Math.round(Number(loss) * 10) / 10;
    if (!Number.isFinite(rounded)) return "-";
    return rounded <= 0 ? "0" : stringify(rounded);
  }

  function hpFinalText(search) {
    const finalHp = firstDefined(search && search.predicted_final_hp, search && search.diagnostics && search.diagnostics.rust_final_hp, null);
    if (finalHp === null || finalHp === undefined) return "-";
    const rounded = Math.round(Number(finalHp) * 10) / 10;
    return Number.isFinite(rounded) ? stringify(rounded) : "-";
  }

  function lifecycleFromPayload(lifecycle) {
    if (!lifecycle || !lifecycle.status) return app.lifecycle || { kind: "Ready" };
    const status = String(lifecycle.status).toLowerCase();
    const base = {
      commandId: firstDefined(lifecycle.command_id, lifecycle.commandId, null),
      sourceStateId: firstDefined(lifecycle.source_state_id, lifecycle.sourceStateId, null),
      expectedStateId: firstDefined(lifecycle.expected_state_id, lifecycle.expectedStateId, null),
      previousStateId: firstDefined(lifecycle.previous_state_id, lifecycle.previousStateId, null),
      raw: lifecycle,
    };
    if (status === "applied") {
      return Object.assign(base, { kind: "Applied", stateId: firstDefined(lifecycle.resulting_state_id, lifecycle.resultingStateId) });
    }
    if (status === "restored") {
      return Object.assign(base, { kind: "Restored", stateId: firstDefined(lifecycle.resulting_state_id, lifecycle.resultingStateId) });
    }
    if (status === "stale") {
      return Object.assign(base, { kind: "Stale", error: lifecycle.error });
    }
    if (status === "rejected") {
      return Object.assign(base, { kind: "Rejected", error: lifecycle.error });
    }
    return Object.assign(base, { kind: "Ready" });
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

  function bridgeStateId() {
    return firstDefined(app.bridge && app.bridge.state_id, app.bridge && app.bridge.stateId, null);
  }

  function hasLoadedSnapshotJson() {
    return Boolean(app.snapshot && app.snapshot.snapshot_json);
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
    if (!app.state) {
      return app.viewMode === "live"
        ? "Attach the current live combat state to inspect simulator advice."
        : "Start a fixture to inspect simulator state.";
    }
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
    const command = app.lifecycle.commandId ? ` #${String(app.lifecycle.commandId).slice(0, 8)}` : "";
    if (app.lifecycle.kind === "Submitting") return `${app.lifecycle.label || "Submitting"}${command}`;
    if (app.lifecycle.kind === "Applied") return app.lifecycle.stateId ? `Applied${command} to ${app.lifecycle.stateId}` : `Applied${command}`;
    if (app.lifecycle.kind === "Restored") return app.lifecycle.stateId ? `Restored${command} to ${app.lifecycle.stateId}` : `Restored${command}`;
    if (app.lifecycle.kind === "Rejected") return `Rejected${command}`;
    if (app.lifecycle.kind === "Stale") return `Stale${command}`;
    return "Ready";
  }

  function lifecycleClass() {
    if (!app.lifecycle) return "neutral";
    if (app.lifecycle.kind === "Rejected" || app.lifecycle.kind === "Stale") return "bad";
    if (app.lifecycle.kind === "Submitting") return "busy";
    if (app.lifecycle.kind === "Applied" || app.lifecycle.kind === "Restored") return "good";
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
    const max = firstDefined(entity.max_hp, entity.maxHealth, entity.max_health, entity.maxHp, entity.max, null);
    return max === null || max === undefined || max === "undefined" ? current : `${current}/${max}`;
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
