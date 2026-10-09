import { displayActionLabel } from "./names.js";

const LANE_W = 16;
const ROW_H = 32;

export function buildIndex(tree) {
  const byId = new Map(tree.nodes.map((node) => [node.id, node]));
  return { byId, rootId: tree.root_id };
}

export function ancestors(byId, nodeId) {
  const path = [];
  let current = byId.get(nodeId);
  const seen = new Set();
  while (current && !seen.has(current.id)) {
    seen.add(current.id);
    path.push(current);
    current = current.parent_id ? byId.get(current.parent_id) : null;
  }
  return path.reverse();
}

export function preferredPath(byId, rootId, selectedId, preferred) {
  const toSelected = ancestors(byId, selectedId);
  const path = [...toSelected];
  let current = byId.get(selectedId);
  const seen = new Set(path.map((node) => node.id));
  while (current) {
    const childId = preferred[current.id] || (current.child_ids.length === 1 ? current.child_ids[0] : null);
    if (!childId) break;
    const child = byId.get(childId);
    if (!child || seen.has(child.id)) break;
    path.push(child);
    seen.add(child.id);
    current = child;
  }
  return path;
}

export function updatePreferredToward(preferred, byId, selectedId) {
  const next = { ...preferred };
  const chain = ancestors(byId, selectedId);
  for (const node of chain.slice(0, -1)) {
    const child = chain.find((item) => item.parent_id === node.id);
    if (child) next[node.id] = child.id;
  }
  return next;
}

export function selectedOutgoing(outgoing, nodeId, selectedId, preferred, byId) {
  if (!outgoing?.length) return null;
  const preferredId = preferred?.[nodeId];
  if (preferredId) {
    const match = outgoing.find((edge) => edge.child_id === preferredId);
    if (match) return match;
  }
  if (selectedId && byId) {
    const chain = ancestors(byId, selectedId);
    const child = chain.find((item) => item.parent_id === nodeId);
    if (child) {
      const match = outgoing.find((edge) => edge.child_id === child.id);
      if (match) return match;
    }
  }
  if (outgoing.length === 1) return outgoing[0];
  return null;
}

export function descendantCount(node, byId) {
  let total = 0;
  for (const childId of node.child_ids || []) {
    const child = byId.get(childId);
    if (!child) continue;
    total += 1 + descendantCount(child, byId);
  }
  return total;
}

export function layoutGitGraph(nodes, _preferred = {}, collapsed = {}) {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const root = nodes.find((node) => node.parent_id == null);
  if (!root) return { rows: [], laneOf: new Map(), edges: [], laneCount: 1 };

  const laneOf = new Map();
  const rows = [];
  let nextLane = 1;

  function walk(node, lane) {
    laneOf.set(node.id, lane);
    rows.push(node);
    if (collapsed[node.id]) return;
    const kids = (node.child_ids || []).map((id) => byId.get(id)).filter(Boolean);
    kids.forEach((child, index) => {
      walk(child, index === 0 ? lane : nextLane++);
    });
  }

  walk(root, 0);

  const indexOf = new Map(rows.map((node, index) => [node.id, index]));
  const edges = [];
  for (const node of rows) {
    for (const childId of node.child_ids || []) {
      const child = byId.get(childId);
      if (!child || !indexOf.has(child.id)) continue;
      edges.push({
        parentRow: indexOf.get(node.id),
        childRow: indexOf.get(child.id),
        parentLane: laneOf.get(node.id),
        childLane: laneOf.get(child.id),
      });
    }
  }
  return { rows, laneOf, edges, laneCount: Math.max(nextLane, 1) };
}

export function rowConnections(layout, rowIndex) {
  const fromTop = new Set();
  const toBottom = new Set();
  const forks = [];
  for (const edge of layout.edges) {
    if (edge.parentRow < rowIndex && edge.childRow >= rowIndex) fromTop.add(edge.childLane);
    if (edge.parentRow <= rowIndex && edge.childRow > rowIndex) toBottom.add(edge.childLane);
    if (edge.parentRow === rowIndex && edge.parentLane !== edge.childLane) {
      forks.push({ from: edge.parentLane, to: edge.childLane });
      toBottom.add(edge.parentLane);
      toBottom.add(edge.childLane);
    }
  }
  return {
    nodeLane: layout.laneOf.get(layout.rows[rowIndex].id),
    fromTop: [...fromTop].sort((a, b) => a - b),
    toBottom: [...toBottom].sort((a, b) => a - b),
    forks,
  };
}

function svgEl(name, attrs) {
  const node = document.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, String(value));
  return node;
}

function laneX(lane) {
  return lane * LANE_W + LANE_W / 2;
}

function edgeOnPath(layout, edge, selectedPath) {
  const parent = layout.rows[edge.parentRow];
  const child = layout.rows[edge.childRow];
  return selectedPath.has(parent.id) && selectedPath.has(child.id);
}

function renderGraph(layout, rowIndex, selectedPath, selectedId) {
  const node = layout.rows[rowIndex];
  const width = Math.max(layout.laneCount, 1) * LANE_W;
  const mid = ROW_H / 2;
  const svg = svgEl("svg", {
    width,
    height: ROW_H,
    viewBox: `0 0 ${width} ${ROW_H}`,
    class: "graphSvg",
    "aria-hidden": "true",
  });
  for (const edge of layout.edges) {
    const color = edgeOnPath(layout, edge, selectedPath) ? "var(--accent)" : "var(--muted)";
    const childX = laneX(edge.childLane);
    const parentX = laneX(edge.parentLane);
    const through = edge.parentRow < rowIndex && edge.childRow > rowIndex;
    const atParent = edge.parentRow === rowIndex;
    const atChild = edge.childRow === rowIndex;
    if (through) {
      svg.appendChild(svgEl("line", { x1: childX, y1: 0, x2: childX, y2: ROW_H, class: "rail", stroke: color }));
    }
    if (atParent && edge.parentLane === edge.childLane && edge.childRow > rowIndex) {
      svg.appendChild(svgEl("line", { x1: parentX, y1: mid, x2: parentX, y2: ROW_H, class: "rail", stroke: color }));
    }
    if (atParent && edge.parentLane !== edge.childLane) {
      svg.appendChild(svgEl("line", { x1: parentX, y1: mid, x2: childX, y2: mid, class: "rail", stroke: color }));
      svg.appendChild(svgEl("line", { x1: childX, y1: mid, x2: childX, y2: ROW_H, class: "rail", stroke: color }));
    }
    if (atChild && edge.parentRow < rowIndex) {
      svg.appendChild(svgEl("line", { x1: childX, y1: 0, x2: childX, y2: mid, class: "rail", stroke: color }));
    }
  }
  const onPath = selectedPath.has(node.id);
  svg.appendChild(
    svgEl("circle", {
      cx: laneX(layout.laneOf.get(node.id)),
      cy: mid,
      r: 4,
      class: node.id === selectedId ? "dot selected" : onPath ? "dot path" : "dot",
    })
  );
  return svg;
}

function shortLabel(node) {
  let label = displayActionLabel(node.incoming_label || "root");
  label = label.replace(/^Play /, "");
  return label;
}

export function renderTree(target, nodes, selectedId, preferred, collapsed, onToggle) {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const selectedPath = new Set(ancestors(byId, selectedId).map((node) => node.id));
  const hidden = collapsed || {};
  const layout = layoutGitGraph(nodes, preferred || {}, hidden);
  const list = document.createElement("div");
  list.className = "treeList";
  layout.rows.forEach((node, index) => {
    const row = document.createElement("div");
    row.className = "treeRow" + (node.id === selectedId ? " selected" : "") + (selectedPath.has(node.id) ? " onPath" : "");
    row.dataset.nodeId = node.id;
    const graph = document.createElement("span");
    graph.className = "graph";
    graph.style.setProperty("--lanes", String(layout.laneCount));
    graph.append(renderGraph(layout, index, selectedPath, selectedId));
    const kidCount = (node.child_ids || []).length;
    let twist = null;
    if (kidCount) {
      twist = document.createElement("button");
      twist.type = "button";
      twist.className = "collapse";
      const isCollapsed = Boolean(hidden[node.id]);
      const hiddenCount = isCollapsed ? descendantCount(node, byId) : 0;
      twist.textContent = isCollapsed ? `▸${hiddenCount}` : "▾";
      twist.title = isCollapsed ? "Expand branch" : "Collapse branch";
      twist.dataset.collapseId = node.id;
      twist.setAttribute("aria-expanded", isCollapsed ? "false" : "true");
      twist.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        if (typeof onToggle !== "function") throw new Error("collapse handler missing");
        onToggle(node.id);
      });
    } else {
      twist = document.createElement("span");
      twist.className = "collapseSpacer";
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = "node" + (node.id === selectedId ? " selected" : "");
    button.dataset.nodeId = node.id;
    const turn = document.createElement("span");
    turn.className = "turn";
    turn.textContent = `T${node.turn}`;
    const title = document.createElement("span");
    title.className = "nodeLabel";
    title.textContent = shortLabel(node);
    const hp = document.createElement("span");
    hp.className = "hpTag";
    hp.textContent = node.hp == null || node.hp === "" ? "—" : `${node.hp}/${node.max_hp ?? "?"}`;
    button.append(turn, title, hp);
    const marks = [];
    if (node.markers?.turn_end) marks.push("turn-end");
    if (node.markers?.kills?.length) marks.push(`kill×${node.markers.kills.length}`);
    if (node.outcome === "win") marks.push("WIN");
    if (node.outcome === "loss") marks.push("LOSS");
    if ((node.child_ids || []).length > 1) marks.push("fork");
    if (marks.length) {
      const mark = document.createElement("span");
      mark.className = "mark";
      mark.textContent = marks.join(" · ");
      button.append(mark);
    }
    if (node.outcome === "win") button.classList.add("win");
    if (node.outcome === "loss") button.classList.add("loss");
    row.append(graph, twist, button);
    list.append(row);
  });
  target.replaceChildren(list);
}

export function jump(path, selectedId, predicate, direction) {
  const index = path.findIndex((node) => node.id === selectedId);
  if (index < 0) return null;
  if (direction > 0) {
    return path.slice(index + 1).find(predicate) || null;
  }
  return [...path.slice(0, index)].reverse().find(predicate) || null;
}
