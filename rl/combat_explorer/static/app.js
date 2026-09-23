import {
  ApiError,
  analysisMatchesRequest,
  api,
  requestId,
  shouldApply,
  shouldApplyAnalysis,
  shouldAssignJob,
  shouldAssignTree,
} from "./api.js";
import { renderActions, renderAnalysisNotes, renderBoard, renderSummary } from "./combat.js";
import { ancestors, buildIndex, descendantCount, jump, preferredPath, renderTree, selectedOutgoing, updatePreferredToward } from "./history.js";

const state = {
  tree: null,
  selectedId: null,
  preferred: {},
  collapsed: {},
  node: null,
  job: null,
  capabilities: null,
  generation: 0,
  sessionGeneration: 0,
  analysisId: 0,
  replaceId: 0,
  inflight: false,
};

const els = {
  rootList: document.querySelector("#rootList"),
  rootQuery: document.querySelector("#rootQuery"),
  rootForm: document.querySelector("#rootForm"),
  generateForm: document.querySelector("#generateForm"),
  modelForm: document.querySelector("#modelForm"),
  mode: document.querySelector("#mode"),
  temperature: document.querySelector("#temperature"),
  maxDecisions: document.querySelector("#maxDecisions"),
  modelStep: document.querySelector("#modelStep"),
  modelFinish: document.querySelector("#modelFinish"),
  cancelJob: document.querySelector("#cancelJob"),
  save: document.querySelector("#save"),
  load: document.querySelector("#load"),
  tree: document.querySelector("#tree"),
  board: document.querySelector("#board"),
  actionList: document.querySelector("#actionList"),
  analysisNote: document.querySelector("#analysisNote"),
  summary: document.querySelector("#summary"),
  error: document.querySelector("#error"),
  scenarioLabel: document.querySelector("#scenarioLabel"),
  modelLabel: document.querySelector("#modelLabel"),
  jobLabel: document.querySelector("#jobLabel"),
  jumpResult: document.querySelector("#jumpResult"),
  startMenu: document.querySelector("#startMenu"),
  menuButton: document.querySelector("#menuButton"),
  menuClose: document.querySelector("#menuClose"),
  menuBackdrop: document.querySelector("#menuBackdrop"),
  scrubber: document.querySelector("#scrubber"),
  pathMeta: document.querySelector("#pathMeta"),
  prev: document.querySelector("#prev"),
  next: document.querySelector("#next"),
  prevTurn: document.querySelector("#prevTurn"),
  nextTurn: document.querySelector("#nextTurn"),
  prevKill: document.querySelector("#prevKill"),
  nextKill: document.querySelector("#nextKill"),
  deleteNode: document.querySelector("#deleteNode"),
  probNote: document.querySelector("#probNote"),
};

window.__explorer = state;

function setMenuOpen(open) {
  document.body.classList.toggle("menu-open", open);
  if (els.startMenu) els.startMenu.classList.toggle("open", open);
  if (els.menuBackdrop) els.menuBackdrop.hidden = !open;
  if (els.menuButton) {
    els.menuButton.setAttribute("aria-expanded", open ? "true" : "false");
    els.menuButton.setAttribute("aria-label", open ? "Close combat menu" : "Open combat menu");
  }
}

function showError(error) {
  if (!error) {
    els.error.hidden = true;
    els.error.textContent = "";
    return;
  }
  const code = error.code ? ` [${error.code}]` : "";
  els.error.hidden = false;
  els.error.textContent = `${error.message || error}${code}`;
}

function busy() {
  return Boolean(state.job && state.job.status === "running");
}

function canMutate() {
  return Boolean(state.node && state.node.id === state.selectedId && !state.node.terminal && !busy() && !state.inflight);
}

function stillSession(sessionId, sessionGeneration) {
  return state.tree?.session_id === sessionId && state.sessionGeneration === sessionGeneration;
}

function stillReplace(replaceId) {
  return state.replaceId === replaceId;
}

function beginReplace() {
  state.replaceId += 1;
  return state.replaceId;
}

function currentSessionMeta() {
  return {
    sessionId: state.tree?.session_id,
    sessionGeneration: state.sessionGeneration,
    jobId: state.job?.id,
  };
}

function assignTree(tree, sessionId, sessionGeneration) {
  if (!shouldAssignTree(tree, sessionId, sessionGeneration, currentSessionMeta())) return false;
  state.tree = tree;
  return true;
}

function assignJob(job, sessionId, sessionGeneration, jobId, replace = false) {
  if (!shouldAssignJob(job, sessionId, sessionGeneration, jobId, currentSessionMeta(), replace)) return false;
  state.job = job;
  return true;
}

function analysisControls() {
  return {
    mode: els.mode.value,
    temperature: Number(els.temperature.value),
    checkpoint: state.tree?.model?.fingerprint || null,
  };
}

function currentApply(meta) {
  return {
    generation: meta.generation,
    sessionId: meta.sessionId,
    sessionGeneration: meta.sessionGeneration,
    nodeId: meta.nodeId,
  };
}

function currentPath() {
  if (!state.tree || !state.selectedId) return [];
  const { byId, rootId } = buildIndex(state.tree);
  return preferredPath(byId, rootId, state.selectedId, state.preferred);
}

function currentOutgoing() {
  if (!state.node || !state.tree) return null;
  const { byId } = buildIndex(state.tree);
  return selectedOutgoing(state.node.outgoing, state.node.id, state.selectedId, state.preferred, byId);
}

async function refreshRoots() {
  const data = await api(`/api/roots?q=${encodeURIComponent(els.rootQuery.value)}`);
  els.rootList.replaceChildren();
  for (const item of data.cases) {
    const option = document.createElement("option");
    option.value = item.id;
    option.textContent = `${item.id} · ${item.label} · A${item.act} F${item.floor} ${item.encounter} HP ${item.hp}/${item.max_hp}`;
    els.rootList.append(option);
  }
}

async function loadCapabilities() {
  state.capabilities = await api("/api/capabilities");
  if (els.maxDecisions && state.capabilities.default_max_decisions) {
    els.maxDecisions.value = String(state.capabilities.default_max_decisions);
    els.maxDecisions.max = String(state.capabilities.max_decisions_limit || 512);
  }
  updateHeader();
}

function updateHeader() {
  const provenance = state.tree?.provenance;
  const board = state.node?.board;
  if (board?.context) {
    const ctx = board.context;
    const name = provenance?.encounter || provenance?.case_id || "Combat";
    els.scenarioLabel.textContent = `${name} · A${ctx.act} F${ctx.floor} · HP ${ctx.player_hp}/${ctx.player_max_hp}`;
  } else if (provenance) {
    els.scenarioLabel.textContent = `${provenance.encounter || provenance.case_id || provenance.kind}`;
  } else {
    els.scenarioLabel.textContent = "No combat loaded";
  }
  const model = state.tree?.model || state.capabilities?.checkpoint_loaded;
  els.modelLabel.textContent = model ? "Model loaded" : "No model";
  const job = state.job && state.tree && state.job.session_id === state.tree.session_id ? state.job : null;
  if (job) {
    const err = job.error ? ` · ${job.error}` : "";
    els.jobLabel.textContent = `Job ${job.status}${job.reason ? " / " + job.reason : ""} · ${job.generated} steps${err}`;
    els.cancelJob.disabled = job.status !== "running";
    els.jumpResult.hidden = !job.leaf_id || job.status === "running";
  } else {
    els.jobLabel.textContent = "";
    els.cancelJob.disabled = true;
    els.jumpResult.hidden = true;
  }
  els.modelStep.disabled = !canMutate() || !state.tree?.model;
  els.modelFinish.disabled = !canMutate() || !state.tree?.model;
  if (els.deleteNode) els.deleteNode.disabled = !canDelete();
}

function canDelete() {
  return Boolean(
    state.tree &&
      state.selectedId &&
      state.selectedId !== state.tree.root_id &&
      !busy() &&
      !state.inflight
  );
}

async function selectNode(nodeId, { analyze = true } = {}) {
  if (!state.tree) return;
  const sessionId = state.tree.session_id;
  const sessionGeneration = state.sessionGeneration;
  const generation = ++state.generation;
  state.analysisId += 1;
  state.selectedId = nodeId;
  const { byId } = buildIndex(state.tree);
  state.preferred = updatePreferredToward(state.preferred, byId, nodeId);
  if (state.node?.id !== nodeId) {
    state.node = null;
  }
  render();
  try {
    const node = await api(`/api/sessions/${sessionId}/nodes/${nodeId}`);
    if (
      !shouldApply(currentApply({ generation, sessionId, sessionGeneration, nodeId }), {
        generation: state.generation,
        sessionId: state.tree?.session_id,
        sessionGeneration: state.sessionGeneration,
        selectedId: state.selectedId,
      })
    ) {
      return;
    }
    state.node = { ...node, current_analysis: null, current_analysis_error: null, analysis_pending: false };
    render();
    if (analyze && state.tree.model && !node.terminal) {
      await refreshAnalysis({ generation, sessionId, sessionGeneration, nodeId });
    }
  } catch (error) {
    if (
      !shouldApply(currentApply({ generation, sessionId, sessionGeneration, nodeId }), {
        generation: state.generation,
        sessionId: state.tree?.session_id,
        sessionGeneration: state.sessionGeneration,
        selectedId: state.selectedId,
      })
    ) {
      return;
    }
    showError(error);
  }
}

async function refreshAnalysis(token) {
  if (!state.tree?.model || !state.node || state.node.terminal) return;
  const sessionId = token?.sessionId || state.tree.session_id;
  const sessionGeneration = token?.sessionGeneration ?? state.sessionGeneration;
  const nodeId = token?.nodeId || state.node.id;
  const generation = token?.generation || state.generation;
  const controls = analysisControls();
  const analysisId = ++state.analysisId;
  if (state.node?.id === nodeId) {
    state.node = { ...state.node, current_analysis: null, current_analysis_error: null, analysis_pending: true };
    render();
  }
  try {
    const analysis = await api(`/api/sessions/${sessionId}/nodes/${nodeId}/analyze`, {
      method: "POST",
      body: { mode: controls.mode, temperature: controls.temperature },
    });
    if (
      !shouldApplyAnalysis(
        { generation, sessionId, sessionGeneration, nodeId, analysisId, ...controls },
        {
          generation: state.generation,
          sessionId: state.tree?.session_id,
          sessionGeneration: state.sessionGeneration,
          selectedId: state.selectedId,
          analysisId: state.analysisId,
          ...analysisControls(),
        }
      )
    ) {
      return;
    }
    if (!analysisMatchesRequest(analysis, controls)) return;
    if (state.node?.id === nodeId) {
      state.node = { ...state.node, current_analysis: analysis, current_analysis_error: null, analysis_pending: false };
      render();
    }
  } catch (error) {
    if (
      !shouldApplyAnalysis(
        { generation, sessionId, sessionGeneration, nodeId, analysisId, ...controls },
        {
          generation: state.generation,
          sessionId: state.tree?.session_id,
          sessionGeneration: state.sessionGeneration,
          selectedId: state.selectedId,
          analysisId: state.analysisId,
          ...analysisControls(),
        }
      )
    ) {
      return;
    }
    if (state.node?.id === nodeId) {
      state.node = { ...state.node, current_analysis: null, current_analysis_error: String(error.message || error), analysis_pending: false };
      render();
      showError(error);
    }
  }
}

async function deleteSelected() {
  showError(null);
  if (!canDelete()) return;
  const sessionId = state.tree.session_id;
  const nodeId = state.selectedId;
  const { byId } = buildIndex(state.tree);
  const node = byId.get(nodeId);
  const hidden = node ? 1 + descendantCount(node, byId) : 1;
  if (hidden > 1 && !window.confirm(`Delete this branch (${hidden} nodes)?`)) return;
  await withInflight(async () => {
    const result = await api(`/api/sessions/${sessionId}/nodes/${nodeId}`, { method: "DELETE" });
    if (state.tree?.session_id !== sessionId) return;
    const deleted = new Set(result.deleted || [nodeId]);
    state.preferred = Object.fromEntries(
      Object.entries(state.preferred).filter(([parent, child]) => !deleted.has(parent) && !deleted.has(child))
    );
    state.collapsed = Object.fromEntries(Object.entries(state.collapsed).filter(([id]) => !deleted.has(id)));
    const tree = await api(`/api/sessions/${sessionId}/tree`);
    if (state.tree?.session_id !== sessionId) return;
    state.tree = tree;
    if (state.job && !tree.jobs?.some((job) => job.id === state.job.id)) state.job = null;
    const nextId = result.parent_id && tree.nodes.some((item) => item.id === result.parent_id) ? result.parent_id : tree.root_id;
    await selectNode(nextId);
  });
}

function toggleCollapsed(id) {
  const nextCollapsed = { ...state.collapsed, [id]: !state.collapsed[id] };
  if (nextCollapsed[id] && state.selectedId && state.tree) {
    const { byId } = buildIndex(state.tree);
    const under = ancestors(byId, state.selectedId).some((node) => node.id === id);
    if (under && state.selectedId !== id) {
      state.collapsed = nextCollapsed;
      selectNode(id).catch(showError);
      return;
    }
  }
  state.collapsed = nextCollapsed;
  render();
}

state.toggleCollapsed = toggleCollapsed;

function render() {
  if (!state.tree) {
    updateHeader();
    return;
  }
  renderTree(els.tree, state.tree.nodes, state.selectedId, state.preferred, state.collapsed, toggleCollapsed);
  const path = currentPath();
  const index = path.findIndex((node) => node.id === state.selectedId);
  els.scrubber.max = String(Math.max(path.length - 1, 0));
  els.scrubber.value = String(Math.max(index, 0));
  els.pathMeta.textContent = state.node
    ? `Turn ${state.node.turn} · decision ${state.node.depth} · revision ${state.node.revision}`
    : state.selectedId
      ? "Loading selected decision…"
      : "";
  renderBoard(els.board, state.node?.board);
  const outgoing = currentOutgoing();
  const pending = Boolean(state.node?.analysis_pending);
  const analysisError = state.node?.current_analysis_error;
  const analysis = pending || analysisError ? null : state.node?.current_analysis;
  renderAnalysisNotes(els.analysisNote, analysis, outgoing, outgoing?.diagnostics, {
    pending,
    error: analysisError,
  });
  if (els.probNote) {
    if (pending) {
      els.probNote.textContent = "Updating the current ranking… Historical numbers stay on the taken move.";
    } else if (analysisError) {
      els.probNote.textContent = `Current analysis failed: ${analysisError}. Historical probabilities are unchanged.`;
    } else if (!state.tree.model) {
      els.probNote.textContent = "No checkpoint loaded. You can still play; historical percents stay if a move was recorded.";
    } else {
      els.probNote.textContent = "Percents are the model’s current ranking. History keeps the numbers from when a move was taken.";
    }
  }
  renderActions(els.actionList, state.node?.actions, analysis, outgoing, (action) => act(action), canMutate());
  renderSummary(els.summary, state.node?.summary, state.node?.markers);
  updateHeader();
}

async function setTree(tree, selectedId, replaceId) {
  if (replaceId !== undefined && !stillReplace(replaceId)) return;
  const sameSession = state.tree?.session_id === tree.session_id;
  state.sessionGeneration += 1;
  state.generation += 1;
  state.analysisId += 1;
  state.tree = tree;
  state.preferred = tree.ui?.preferred_children || (sameSession ? state.preferred : {});
  state.collapsed = sameSession ? state.collapsed : {};
  if (!sameSession) {
    state.job = tree.busy && tree.busy.status === "running" ? tree.busy : null;
    state.node = null;
    setMenuOpen(false);
  }
  const nextId = selectedId || tree.ui?.selected_node_id || tree.root_id;
  await selectNode(nextId);
}

async function withInflight(fn) {
  if (state.inflight) return;
  state.inflight = true;
  render();
  try {
    await fn();
  } finally {
    state.inflight = false;
    render();
  }
}

async function act(action) {
  showError(null);
  if (!canMutate()) return;
  const sessionId = state.tree.session_id;
  const sessionGeneration = state.sessionGeneration;
  const nodeId = state.node.id;
  const revision = state.node.revision;
  await withInflight(async () => {
    try {
      const result = await api(`/api/sessions/${sessionId}/nodes/${nodeId}/act`, {
        method: "POST",
        body: {
          native_index: action.native_index,
          revision,
          descriptor: action.descriptor,
          request_id: requestId(),
        },
      });
      if (!stillSession(sessionId, sessionGeneration)) return;
      const tree = await api(`/api/sessions/${sessionId}/tree`);
      if (!assignTree(tree, sessionId, sessionGeneration)) return;
      if (state.selectedId === nodeId) {
        await selectNode(result.child_id);
      } else {
        render();
      }
    } catch (error) {
      if (stillSession(sessionId, sessionGeneration)) showError(error);
    }
  });
}

async function modelStep() {
  showError(null);
  if (!canMutate()) return;
  const sessionId = state.tree.session_id;
  const sessionGeneration = state.sessionGeneration;
  const nodeId = state.node.id;
  await withInflight(async () => {
    try {
      const result = await api(`/api/sessions/${sessionId}/nodes/${nodeId}/model-step`, {
        method: "POST",
        body: {
          mode: els.mode.value,
          temperature: Number(els.temperature.value),
          request_id: requestId(),
        },
      });
      if (!stillSession(sessionId, sessionGeneration)) return;
      const tree = await api(`/api/sessions/${sessionId}/tree`);
      if (!assignTree(tree, sessionId, sessionGeneration)) return;
      if (state.selectedId === nodeId) {
        await selectNode(result.child_id);
      } else {
        render();
      }
    } catch (error) {
      if (stillSession(sessionId, sessionGeneration)) showError(error);
    }
  });
}

async function modelFinish() {
  showError(null);
  if (!canMutate()) return;
  const sessionId = state.tree.session_id;
  const source = state.selectedId;
  const sessionGeneration = state.sessionGeneration;
  await withInflight(async () => {
    try {
      const job = await api(`/api/sessions/${sessionId}/nodes/${source}/continue`, {
        method: "POST",
        body: {
          mode: els.mode.value,
          temperature: Number(els.temperature.value),
          max_decisions: Number(els.maxDecisions.value),
          request_id: requestId(),
        },
      });
      if (!assignJob(job, sessionId, sessionGeneration, job?.id, true)) return;
      updateHeader();
      pollJob(job.id, sessionId, sessionGeneration);
    } catch (error) {
      if (stillSession(sessionId, sessionGeneration)) showError(error);
    }
  });
}

async function pollJob(jobId, sessionId, sessionGeneration) {
  if (!jobId || !sessionId) return;
  try {
    const job = await api(`/api/jobs/${jobId}`);
    if (!assignJob(job, sessionId, sessionGeneration, jobId)) return;
    const tree = await api(`/api/sessions/${sessionId}/tree`);
    if (!assignTree(tree, sessionId, sessionGeneration)) return;
    updateHeader();
    if (job.status === "running") {
      window.setTimeout(() => pollJob(jobId, sessionId, sessionGeneration), 400);
      return;
    }
    if (job.error) showError(new ApiError(job.error, { code: job.error_code || job.reason }));
    render();
  } catch (error) {
    if (!stillSession(sessionId, sessionGeneration)) return;
    showError(error);
  }
}

function move(delta) {
  const path = currentPath();
  const index = path.findIndex((node) => node.id === state.selectedId);
  const next = path[index + delta];
  if (next) selectNode(next.id).catch(showError);
}

function jumpMarker(predicate, direction) {
  const target = jump(currentPath(), state.selectedId, predicate, direction);
  if (target) selectNode(target.id).catch(showError);
}

els.menuButton?.addEventListener("click", () => setMenuOpen(!document.body.classList.contains("menu-open")));
els.menuClose?.addEventListener("click", () => setMenuOpen(false));
els.menuBackdrop?.addEventListener("click", () => setMenuOpen(false));
els.rootQuery.addEventListener("input", () => refreshRoots().catch(showError));
els.rootList.addEventListener("dblclick", () => {
  if (els.rootList.value) els.rootForm.requestSubmit();
});
els.rootForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  showError(null);
  const replaceId = beginReplace();
  try {
    const caseId = els.rootList.value;
    if (!caseId) throw new Error("Select a validation case");
    const tree = await api("/api/sessions/from-root", { method: "POST", body: { case_id: caseId, request_id: requestId() } });
    if (!stillReplace(replaceId)) return;
    await setTree(tree, tree.root_id, replaceId);
  } catch (error) {
    if (stillReplace(replaceId)) showError(error);
  }
});
els.generateForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  showError(null);
  const replaceId = beginReplace();
  try {
    const floorValue = document.querySelector("#genFloor").value;
    const tree = await api("/api/sessions/generated", {
      method: "POST",
      body: {
        generation_seed: document.querySelector("#genSeed").value,
        floor: floorValue === "" ? null : Number(floorValue),
        min_floor: Number(document.querySelector("#genMin").value),
        max_floor: Number(document.querySelector("#genMax").value),
        request_id: requestId(),
      },
    });
    if (!stillReplace(replaceId)) return;
    await setTree(tree, tree.root_id, replaceId);
  } catch (error) {
    if (stillReplace(replaceId)) showError(error);
  }
});
els.modelForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  showError(null);
  const sessionId = state.tree?.session_id;
  const sessionGeneration = state.sessionGeneration;
  try {
    if (!state.tree) throw new Error("Start a session first");
    const path = document.querySelector("#modelPath").value;
    const selectedId = state.selectedId;
    await api(`/api/sessions/${sessionId}/model`, { method: "POST", body: { path } });
    if (!stillSession(sessionId, sessionGeneration)) return;
    const tree = await api(`/api/sessions/${sessionId}/tree`);
    if (!assignTree(tree, sessionId, sessionGeneration)) return;
    await selectNode(state.selectedId || selectedId);
  } catch (error) {
    if (stillSession(sessionId, sessionGeneration)) showError(error);
  }
});
els.modelStep.addEventListener("click", () => modelStep().catch(showError));
els.modelFinish.addEventListener("click", () => modelFinish().catch(showError));
els.mode.addEventListener("change", () => refreshAnalysis().catch(showError));
els.temperature.addEventListener("change", () => refreshAnalysis().catch(showError));
els.cancelJob.addEventListener("click", async () => {
  if (state.job && state.tree && state.job.session_id === state.tree.session_id) {
    const jobId = state.job.id;
    const sessionId = state.tree.session_id;
    const sessionGeneration = state.sessionGeneration;
    try {
      const job = await api(`/api/jobs/${jobId}/cancel`, { method: "POST" });
      if (!assignJob(job, sessionId, sessionGeneration, jobId)) return;
      updateHeader();
    } catch (error) {
      if (stillSession(sessionId, sessionGeneration)) showError(error);
    }
  }
});
els.save.addEventListener("click", async () => {
  showError(null);
  try {
    if (!state.tree) throw new Error("Start a session first");
    const filename = window.prompt("Save filename", `${state.tree.session_id}.json`);
    if (!filename) return;
    await api(`/api/sessions/${state.tree.session_id}/save`, {
      method: "POST",
      body: { filename, selected_node_id: state.selectedId, preferred_children: state.preferred },
    });
  } catch (error) {
    showError(error);
  }
});
els.load.addEventListener("click", async () => {
  showError(null);
  let replaceId = null;
  try {
    const filename = window.prompt("Load filename");
    if (!filename) return;
    replaceId = beginReplace();
    const tree = await api("/api/sessions/load", { method: "POST", body: { filename } });
    if (!stillReplace(replaceId)) return;
    await setTree(tree, tree.ui?.selected_node_id || tree.root_id, replaceId);
  } catch (error) {
    if (replaceId == null || stillReplace(replaceId)) showError(error);
  }
});
els.tree.addEventListener("click", (event) => {
  if (event.target.closest("button.collapse")) return;
  const row = event.target.closest(".treeRow[data-node-id]");
  if (row?.dataset.nodeId) selectNode(row.dataset.nodeId).catch(showError);
});
els.prev.addEventListener("click", () => move(-1));
els.next.addEventListener("click", () => move(1));
els.prevTurn.addEventListener("click", () => jumpMarker((node) => node.markers?.turn_end || node.depth === 0, -1));
els.nextTurn.addEventListener("click", () => jumpMarker((node) => node.markers?.turn_end, 1));
els.prevKill.addEventListener("click", () => jumpMarker((node) => node.markers?.kills?.length, -1));
els.nextKill.addEventListener("click", () => jumpMarker((node) => node.markers?.kills?.length, 1));
els.deleteNode?.addEventListener("click", () => deleteSelected().catch(showError));
els.jumpResult.addEventListener("click", () => {
  if (state.job?.leaf_id && state.tree && state.job.session_id === state.tree.session_id) {
    selectNode(state.job.leaf_id).catch(showError);
  }
});
els.scrubber.addEventListener("input", () => {
  const path = currentPath();
  const node = path[Number(els.scrubber.value)];
  if (node) selectNode(node.id).catch(showError);
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && document.body.classList.contains("menu-open")) {
    setMenuOpen(false);
    return;
  }
  const tag = event.target.tagName;
  if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
  if (document.body.classList.contains("menu-open")) return;
  if (event.key === "ArrowLeft") {
    event.preventDefault();
    move(-1);
  } else if (event.key === "ArrowRight") {
    event.preventDefault();
    move(1);
  } else if (event.key === "Delete" || event.key === "Backspace") {
    event.preventDefault();
    deleteSelected().catch(showError);
  }
});

refreshRoots().catch(showError);
loadCapabilities().catch(showError);
window.addEventListener("error", (event) => showError(event.error || event.message));
window.addEventListener("unhandledrejection", (event) => showError(event.reason));
