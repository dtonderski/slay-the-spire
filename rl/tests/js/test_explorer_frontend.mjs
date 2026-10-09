import {
  analysisMatchesRequest,
  probabilityByNative,
  shouldApply,
  shouldApplyAnalysis,
  shouldAssignJob,
  shouldAssignTree,
} from "../../combat_explorer/static/api.js";
import { actionProbabilityView } from "../../combat_explorer/static/combat.js";
import { descendantCount, layoutGitGraph, rowConnections, selectedOutgoing } from "../../combat_explorer/static/history.js";
import { displayActionLabel, displayName } from "../../combat_explorer/static/names.js";

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

assert(
  shouldApply(
    { generation: 1, sessionId: "s", nodeId: "n" },
    { generation: 1, sessionId: "s", selectedId: "n" }
  ),
  "matching generation/session/node should apply"
);
assert(
  !shouldApply(
    { generation: 1, sessionId: "s", nodeId: "n" },
    { generation: 2, sessionId: "s", selectedId: "n" }
  ),
  "stale generation must not apply"
);
assert(
  !shouldApply(
    { generation: 1, sessionId: "s", nodeId: "n" },
    { generation: 1, sessionId: "other", selectedId: "n" }
  ),
  "foreign session must not apply"
);
assert(
  !shouldApply(
    { generation: 1, sessionId: "s", nodeId: "n1" },
    { generation: 1, sessionId: "s", selectedId: "n2" }
  ),
  "stale node must not apply"
);
assert(
  !shouldApply(
    { generation: 1, sessionId: "s", jobId: "j1" },
    { generation: 1, sessionId: "s", selectedId: "n", jobId: "j2" }
  ),
  "foreign job must not apply"
);

assert(
  shouldApplyAnalysis(
    { generation: 1, sessionId: "s", nodeId: "n", analysisId: 3, mode: "sample", temperature: 0.25, checkpoint: "fp" },
    { generation: 1, sessionId: "s", selectedId: "n", analysisId: 3, mode: "sample", temperature: 0.25, checkpoint: "fp" }
  ),
  "matching analysis request should apply"
);
assert(
  !shouldApplyAnalysis(
    { generation: 1, sessionId: "s", nodeId: "n", analysisId: 3, mode: "sample", temperature: 0.25, checkpoint: "fp" },
    { generation: 1, sessionId: "s", selectedId: "n", analysisId: 4, mode: "sample", temperature: 0.25, checkpoint: "fp" }
  ),
  "stale analysisId must not apply"
);
assert(
  !shouldApplyAnalysis(
    { generation: 1, sessionId: "s", nodeId: "n", analysisId: 3, mode: "sample", temperature: 0.5, checkpoint: "fp" },
    { generation: 1, sessionId: "s", selectedId: "n", analysisId: 3, mode: "sample", temperature: 0.25, checkpoint: "fp" }
  ),
  "stale temperature must not apply"
);
assert(
  !shouldApplyAnalysis(
    { generation: 1, sessionId: "s", nodeId: "n", analysisId: 3, mode: "sample", temperature: 0.25, checkpoint: "fp" },
    { generation: 1, sessionId: "s", selectedId: "n", analysisId: 3, mode: "greedy", temperature: 0.25, checkpoint: "fp" }
  ),
  "stale mode must not apply"
);
assert(
  !shouldApplyAnalysis(
    { generation: 1, sessionId: "s", nodeId: "n", analysisId: 3, mode: "sample", temperature: 0.25, checkpoint: "old" },
    { generation: 1, sessionId: "s", selectedId: "n", analysisId: 3, mode: "sample", temperature: 0.25, checkpoint: "new" }
  ),
  "stale checkpoint must not apply"
);

assert(
  analysisMatchesRequest(
    { historical: false, mode: "sample", temperature: 0.25, checkpoint_fingerprint: "fp" },
    { mode: "sample", temperature: 0.25, checkpoint: "fp" }
  ),
  "analysis payload must match requested sample settings"
);
assert(
  !analysisMatchesRequest(
    { historical: false, mode: "sample", temperature: 0.5, checkpoint_fingerprint: "fp" },
    { mode: "sample", temperature: 0.25, checkpoint: "fp" }
  ),
  "inverted temperature payload must not be labeled current"
);
assert(
  !analysisMatchesRequest(
    { historical: true, mode: "sample", temperature: 0.25 },
    { mode: "sample", temperature: 0.25 }
  ),
  "historical diagnostics must not be treated as current reanalysis"
);
assert(
  analysisMatchesRequest({ historical: false, mode: "greedy", temperature: null }, { mode: "greedy", temperature: 1 }),
  "greedy analysis has null temperature"
);
assert(
  !analysisMatchesRequest({ historical: false, mode: "greedy", temperature: 1 }, { mode: "greedy", temperature: 1 }),
  "greedy payload with a temperature is not current greedy reanalysis"
);

assert(
  shouldAssignTree({ session_id: "a" }, "a", 2, { sessionId: "a", sessionGeneration: 2 }),
  "tree assignment requires the live session"
);
assert(
  !shouldAssignTree({ session_id: "a" }, "a", 2, { sessionId: "b", sessionGeneration: 3 }),
  "foreign session tree must not overwrite the newly selected session"
);
assert(
  !shouldAssignTree({ session_id: "b" }, "a", 2, { sessionId: "a", sessionGeneration: 2 }),
  "tree payload session_id must match the request"
);
assert(
  shouldAssignJob({ id: "j1", session_id: "a" }, "a", 1, "j1", { sessionId: "a", sessionGeneration: 1, jobId: "j1" }),
  "matching job may be assigned"
);
assert(
  !shouldAssignJob({ id: "j1", session_id: "a" }, "a", 1, "j1", { sessionId: "b", sessionGeneration: 2, jobId: null }),
  "cancel/poll for a previous session must not assign onto the new session"
);
assert(
  !shouldAssignJob({ id: "j2", session_id: "a" }, "a", 1, "j1", { sessionId: "a", sessionGeneration: 1, jobId: "j1" }),
  "job id mismatch must not assign"
);
assert(
  !shouldAssignJob({ id: "j2", session_id: "a" }, "a", 1, "j2", { sessionId: "a", sessionGeneration: 1, jobId: "j1" }),
  "stale poll/cancel must not replace a different live job"
);
assert(
  shouldAssignJob({ id: "j2", session_id: "a" }, "a", 1, "j2", { sessionId: "a", sessionGeneration: 1, jobId: "j1" }, true),
  "starting a new job may replace a completed job on the same session"
);

const outgoing = [
  { child_id: "c1", native_index: 2, label: "End turn", actor: "human" },
  { child_id: "c2", native_index: 0, label: "Play Strike", actor: "model" },
];
const byId = new Map([
  ["root", { id: "root", parent_id: null, child_ids: ["c1", "c2"] }],
  ["c1", { id: "c1", parent_id: "root", child_ids: [] }],
  ["c2", { id: "c2", parent_id: "root", child_ids: [] }],
]);
const selected = selectedOutgoing(outgoing, "root", "c2", { root: "c2" }, byId);
assert(selected?.child_id === "c2", "preferred child should be selected outgoing");
assert(selected?.native_index === 0, "outgoing native index is the child's incoming action, not the parent incoming index");
assert(selectedOutgoing(outgoing, "root", "c1", {}, byId)?.native_index === 2, "ancestor path selects the matching child");

const historical = {
  native_indices: [4, 7, 9],
  labels: ["Strike", "Defend", "End turn"],
  base_probabilities: [0.5, 0.25, 0.25],
  adjusted_probabilities: [0.8, 0.1, 0.1],
  mode: "sample",
  temperature: 0.25,
  historical: true,
};
const mapped = probabilityByNative(historical);
assert(mapped.get(7)?.base === 0.25, "historical base maps through native_indices");
assert(mapped.get(7)?.adjusted === 0.1, "historical adjusted maps through native_indices");
assert(!mapped.has(0), "unmapped native indices are absent");

const view = actionProbabilityView(
  { native_index: 7, allowed: true, label: "Defend" },
  {
    historical: false,
    mode: "sample",
    temperature: 0.5,
    native_indices: [4, 7, 9],
    base_probabilities: [0.4, 0.4, 0.2],
    adjusted_probabilities: [0.2, 0.6, 0.2],
  },
  { native_index: 4, diagnostics: historical }
);
assert(view.histBase === "0.250", "hist p is the recorded base probability for native 7");
assert(view.histAdj === "0.100", "hist adj is the recorded adjusted probability for native 7");
assert(view.nowBase === "0.400", "now p is the current reanalysis base");
assert(view.nowAdj === "0.600", "now adj is the current reanalysis adjusted");
assert(view.outgoing === false, "native 7 is not the outgoing action");

const noModel = actionProbabilityView(
  { native_index: 4, allowed: true, label: "Strike" },
  null,
  { native_index: 4, diagnostics: historical }
);
assert(noModel.histBase === "0.500", "historical probabilities render without a loaded model");
assert(noModel.nowBase === null, "current reanalysis is absent without a model");
assert(noModel.outgoing === true, "outgoing action is marked from native_index");

assert(displayName("Strike_R") === "Strike", "class suffix stripped");
assert(displayName("BLOOD_FOR_BLOOD") === "Blood For Blood", "underscored caps become words");
assert(displayName("liquid_bronze") === "Liquid Bronze", "potion keys title-case");
assert(displayActionLabel("Play IMPERVIOUS") === "Play Impervious", "action labels are humanized");
assert(displayActionLabel("Play Strike_R -> Spike Slime Large#0") === "Play Strike -> Spike Slime Large", "targets drop slot suffixes");

const graphNodes = [
  { id: "root", parent_id: null, child_ids: ["main", "side"] },
  { id: "main", parent_id: "root", child_ids: ["main2"] },
  { id: "main2", parent_id: "main", child_ids: [] },
  { id: "side", parent_id: "root", child_ids: ["side2"] },
  { id: "side2", parent_id: "side", child_ids: [] },
];
const graph = layoutGitGraph(graphNodes, { root: "main" });
const clickedSide = layoutGitGraph(graphNodes, { root: "side" });
assert(graph.rows.map((node) => node.id).join(",") === "root,main,main2,side,side2", "child creation order is stable");
assert(
  graph.rows.map((node) => node.id).join(",") === clickedSide.rows.map((node) => node.id).join(","),
  "clicking another branch must not reorder rows"
);
assert(graph.laneOf.get("side") === clickedSide.laneOf.get("side"), "clicking another branch must not move lanes");
assert(graph.laneOf.get("root") === 0 && graph.laneOf.get("main") === 0 && graph.laneOf.get("side") === 1, "later children get a side lane");
const rootConn = rowConnections(graph, 0);
assert(rootConn.nodeLane === 0, "root occupies lane 0");
assert(rootConn.toBottom.join(",") === "0,1", "root rails continue into both children");
assert(rootConn.forks.some((fork) => fork.from === 0 && fork.to === 1), "root draws a fork into the side lane");
const sideConn = rowConnections(graph, 3);
assert(sideConn.nodeLane === 1, "side branch is column 1, not indented text");
assert(sideConn.fromTop.includes(1), "side branch is connected from the fork");
const mainConn = rowConnections(graph, 1);
assert(mainConn.nodeLane === 0, "first child continues lane 0");
assert(mainConn.fromTop.includes(0), "first child is connected to root by a rail");
const nested = layoutGitGraph([
  { id: "a", parent_id: null, child_ids: ["b", "d"] },
  { id: "b", parent_id: "a", child_ids: ["c", "e"] },
  { id: "c", parent_id: "b", child_ids: [] },
  { id: "d", parent_id: "a", child_ids: [] },
  { id: "e", parent_id: "b", child_ids: [] },
], {});
assert(nested.laneOf.get("a") === 0 && nested.laneOf.get("b") === 0 && nested.laneOf.get("c") === 0, "first-child spine stays lane 0");
assert(nested.laneOf.get("e") === 1, "b->e gets its own lane");
assert(nested.laneOf.get("d") === 2, "a->d must not reuse b->e's lane");
assert(nested.laneOf.get("d") !== nested.laneOf.get("e"), "nested forks cannot overlap");
const collapsedB = layoutGitGraph(nested.rows.length ? [
  { id: "a", parent_id: null, child_ids: ["b", "d"] },
  { id: "b", parent_id: "a", child_ids: ["c", "e"] },
  { id: "c", parent_id: "b", child_ids: [] },
  { id: "d", parent_id: "a", child_ids: [] },
  { id: "e", parent_id: "b", child_ids: [] },
] : [], {}, { b: true });
assert(collapsedB.rows.map((node) => node.id).join(",") === "a,b,d", "collapsing b hides c and e, not sibling d");
assert(descendantCount({ id: "b", child_ids: ["c", "e"] }, new Map([
  ["c", { id: "c", child_ids: [] }],
  ["e", { id: "e", child_ids: [] }],
])) === 2, "descendantCount includes the hidden subtree");

const linear = layoutGitGraph([
  { id: "a", parent_id: null, child_ids: ["b"] },
  { id: "b", parent_id: "a", child_ids: ["c"] },
  { id: "c", parent_id: "b", child_ids: [] },
], {});
assert(rowConnections(linear, 0).toBottom.includes(0), "linear parent connects downward");
assert(rowConnections(linear, 1).fromTop.includes(0) && rowConnections(linear, 1).toBottom.includes(0), "middle node has a through-rail");
assert(rowConnections(linear, 2).fromTop.includes(0), "leaf is connected from above");

console.log("ok");
