import { probabilityByNative } from "./api.js";
import { displayActionLabel, displayName, monsterTitle } from "./names.js";

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function meter(kind, value, max, label) {
  const wrap = el("div", `meter ${kind}`);
  const fill = el("span", "fill");
  const ratio = max > 0 ? Math.max(0, Math.min(1, value / max)) : 0;
  fill.style.width = `${(ratio * 100).toFixed(1)}%`;
  wrap.append(fill, el("em", null, label));
  return wrap;
}

function cardNode(card, slot) {
  const node = el("div", "card");
  const name = displayName(card?.name || card?.content_key);
  const title = slot === undefined ? name : name;
  const head = el("div", "cardName", title);
  if (slot !== undefined) head.prepend(el("span", "slot", String(slot)));
  node.append(head);
  if (card?.cost !== undefined && card?.cost !== null) {
    node.append(el("div", "cost", String(card.cost) + (card.cost_is_modified ? "*" : "")));
  }
  return node;
}

function powerLine(powers) {
  if (!powers?.length) return "";
  return powers.map((power) => `${displayName(power.key)} ${power.amount}`).join(", ");
}

export function renderBoard(target, board) {
  target.replaceChildren();
  if (!board) {
    target.append(el("p", "muted", "Load a combat to see the board."));
    return;
  }
  const ctx = board.context || {};
  const combat = board.combat;
  const wrap = el("div", "fight boardGrid");
  const meta = [];
  if (ctx.act != null) meta.push(`Act ${ctx.act}`);
  if (ctx.floor != null) meta.push(`Floor ${ctx.floor}`);
  if (ctx.gold != null) meta.push(`${ctx.gold} gold`);
  wrap.append(el("div", "fightMeta", meta.join(" · ")));
  const hp = combat?.player?.hp ?? board.hp ?? ctx.player_hp;
  const maxHp = combat?.player?.max_hp ?? board.max_hp ?? ctx.player_max_hp;
  if (board.outcome === "win" || board.outcome === "loss") {
    const title = board.outcome === "win" ? "Victory" : "Defeat";
    const hpText = hp == null ? title : `${title} · HP ${hp}/${maxHp}`;
    wrap.append(el("div", "banner " + (board.outcome === "win" ? "win" : "loss"), hpText));
  }
  if (board.note && board.outcome !== "win" && board.outcome !== "loss") wrap.append(el("p", "note", board.note));

  const player = combat?.player;
  if (hp != null && maxHp != null) {
    const vitals = el("div", "vitals");
    vitals.append(meter("hp", hp, maxHp, `HP ${hp}/${maxHp}`));
    const blockVal = player?.block ?? 0;
    const block = el("div", "blockChip", `Block ${blockVal}`);
    if (!blockVal) block.classList.add("empty");
    vitals.append(block);
    const energy = el("div", "energy");
    const maxEnergy = player?.max_energy || 0;
    for (let i = 0; i < maxEnergy; i += 1) {
      energy.append(el("span", i < player.energy ? "orb filled" : "orb"));
    }
    if (maxEnergy) energy.append(el("span", "energyLabel", `${player.energy}/${player.max_energy}`));
    vitals.append(energy);
    wrap.append(vitals);
    if (player?.powers?.length) wrap.append(el("div", "powers", powerLine(player.powers)));
  }

  if (combat?.monsters) {
    const row = el("div", "enemies");
    const showSlot = combat.monsters.length > 1;
    for (const monster of combat.monsters) {
      const node = el("div", "monster" + (monster.alive ? "" : " dead"));
      const intent =
        monster.intent?.visibility === "visible"
          ? `${displayName(monster.intent.category)}${monster.intent.damage != null ? ` ${monster.intent.damage}×${monster.intent.hits ?? 1}` : ""}`
          : displayName(monster.intent?.visibility || "intent");
      const extras = [];
      if (monster.minion) extras.push("minion");
      if (!monster.targetable) extras.push("untargetable");
      if (monster.escaped) extras.push("escaped");
      node.append(el("div", "monsterName", monsterTitle(monster) + (showSlot ? ` · ${monster.slot}` : "")));
      node.append(meter("hp", monster.hp, monster.max_hp, `${monster.hp}/${monster.max_hp}`));
      const intentNode = el("div", `intent ${String(monster.intent?.category || "").toLowerCase()}`, intent);
      node.append(intentNode);
      if (monster.block) node.append(el("div", "muted", `Block ${monster.block}`));
      if (extras.length) node.append(el("div", "muted", extras.join(" · ")));
      if (monster.powers?.length) node.append(el("div", "powers", powerLine(monster.powers)));
      row.append(node);
    }
    wrap.append(row);
  }

  if (combat?.hand) {
    const hand = el("div", "row hand");
    hand.append(el("strong", null, "Hand"));
    const cards = el("div", "row");
    for (const entry of combat.hand) cards.append(cardNode(entry.card, entry.slot));
    hand.append(cards);
    wrap.append(hand);
  }

  if (ctx.relics) {
    const row = el("div", "row relics");
    row.append(el("strong", null, "Relics"));
    for (const relic of ctx.relics) row.append(el("div", "relic", displayName(relic.content_key)));
    wrap.append(row);
  }
  if (ctx.potions) {
    const row = el("div", "row potions");
    row.append(el("strong", null, "Potions"));
    for (const potion of ctx.potions) row.append(el("div", "potion" + (potion.content_key ? "" : " empty"), displayName(potion.content_key || "empty")));
    wrap.append(row);
  }

  if (combat) {
    wrap.append(pile("Draw", combat.draw_pile, true));
    wrap.append(pile("Discard", combat.discard_pile, false));
    wrap.append(pile("Exhaust", combat.exhaust_pile, false));
  }

  if (board.selection) {
    wrap.append(el("div", "note", `Selection: ${displayName(board.selection.kind)}`));
    const row = el("div", "row");
    for (const option of board.selection.options) {
      const selected = board.selection.selected_slots.includes(option.slot);
      const card = { ...option.card, name: `${selected ? "✓ " : ""}${displayName(option.card.name)}` };
      row.append(cardNode(card, option.slot));
    }
    wrap.append(row);
  }
  target.append(wrap);
}

function pile(title, pileData, unordered) {
  const count = pileData?.count ?? 0;
  const details = el("details", "pile");
  if (count && pileData?.identities?.length) {
    details.append(el("summary", null, `${title} ${count}${unordered ? " · order unknown" : ""}`));
  } else {
    const wrap = el("div", "pileClosed");
    wrap.append(el("strong", null, `${title} ${count}`));
    return wrap;
  }
  if (unordered && pileData?.known_positions?.length) {
    details.append(
      el(
        "div",
        "muted",
        "Known positions: " + pileData.known_positions.map((item) => `${item.position}:${displayName(item.card.name)}`).join(", ")
      )
    );
  }
  const row = el("div", "row");
  for (const card of pileData.identities) row.append(cardNode(card));
  details.append(row);
  return details;
}

function fmtProb(value) {
  return typeof value === "number" && Number.isFinite(value) ? value.toFixed(3) : null;
}

function pct(value) {
  if (value == null) return null;
  return `${Math.round(Number(value) * 100)}%`;
}

export function actionProbabilityView(action, analysis, outgoing) {
  const currentSource = analysis && !analysis.historical ? analysis : null;
  const currentProbs = probabilityByNative(currentSource);
  const historicalProbs = probabilityByNative(outgoing?.diagnostics);
  const hist = historicalProbs.get(action.native_index);
  const current = currentProbs.get(action.native_index);
  const histAdj = hist && outgoing?.diagnostics?.mode === "sample" ? fmtProb(hist.adjusted) : null;
  const nowAdj = current && currentSource?.mode === "sample" ? fmtProb(current.adjusted) : null;
  return {
    histBase: fmtProb(hist?.base),
    histAdj,
    nowBase: fmtProb(current?.base),
    nowAdj,
    outgoing: action.native_index === outgoing?.native_index,
  };
}

function historicalProbabilityLine(historical) {
  const histMap = probabilityByNative(historical);
  if (!histMap.size) return null;
  const labels = Array.isArray(historical.labels) ? historical.labels : [];
  const parts = [];
  for (const [index, native] of (historical.native_indices || []).entries()) {
    const probs = histMap.get(native);
    if (!probs) continue;
    const name = displayActionLabel(labels[index] || `#${native}`);
    const bits = [name];
    const base = fmtProb(probs.base);
    const adj = fmtProb(probs.adjusted);
    if (base != null) bits.push(`hist p=${base}`);
    if (adj != null && historical.mode === "sample") bits.push(`hist adj=${adj}`);
    parts.push(bits.join(" "));
  }
  return parts.length ? parts.join("; ") : null;
}

export function renderAnalysisNotes(target, analysis, outgoing, historical, status = {}) {
  target.replaceChildren();
  if (historical) {
    const mode = historical.mode || "unknown";
    const temp = historical.mode === "greedy" ? "greedy" : `T=${historical.temperature}`;
    const fp = historical.checkpoint_fingerprint ? historical.checkpoint_fingerprint.slice(0, 12) : "unknown";
    const meta = el(
      "p",
      "note",
      `Taken: ${displayActionLabel(outgoing?.label || "unknown")} · ${mode} ${temp} · ${fp}`
    );
    meta.dataset.kind = "historical";
    target.append(meta);
    const line = historicalProbabilityLine(historical);
    if (line) {
      const probs = el("p", "histProb", `Historical probabilities: ${line}`);
      probs.dataset.kind = "historical-probabilities";
      target.append(probs);
    }
  } else if (outgoing) {
    target.append(el("p", "note", `Branch: ${displayActionLabel(outgoing.label)} (${outgoing.actor || "unknown"}).`));
  }
  if (status.pending) {
    const pending = el("p", "muted", "Updating ranking…");
    pending.dataset.kind = "current-pending";
    target.append(pending);
  } else if (status.error) {
    const failed = el("p", "loss", `Current reanalysis failed: ${status.error}`);
    failed.dataset.kind = "current-failed";
    target.append(failed);
  } else if (analysis && !analysis.historical) {
    const mode = analysis.mode || "unknown";
    const temp = analysis.mode === "greedy" ? "greedy" : `T=${analysis.temperature}`;
    const fp = analysis.checkpoint_fingerprint ? analysis.checkpoint_fingerprint.slice(0, 12) : "unknown";
    const current = el("p", "muted", `Current reanalysis: ${mode} ${temp} · ${fp}`);
    current.dataset.kind = "current";
    current.dataset.mode = mode;
    current.dataset.temperature = analysis.temperature == null ? "" : String(analysis.temperature);
    target.append(current);
  } else if (!analysis) {
    const none = el("p", "muted", "No current ranking. Historical probabilities still display.");
    none.dataset.kind = "current-absent";
    target.append(none);
  }
}

export function renderActions(target, actions, analysis, outgoing, onAct, canAct) {
  target.replaceChildren();
  const currentSource = analysis && !analysis.historical ? analysis : null;
  const currentProbs = probabilityByNative(currentSource);
  const historicalProbs = probabilityByNative(outgoing?.diagnostics);
  const ordered = [...(actions || [])].sort((a, b) => {
    const pa =
      currentProbs.get(a.native_index)?.adjusted ??
      currentProbs.get(a.native_index)?.base ??
      historicalProbs.get(a.native_index)?.adjusted ??
      historicalProbs.get(a.native_index)?.base ??
      -1;
    const pb =
      currentProbs.get(b.native_index)?.adjusted ??
      currentProbs.get(b.native_index)?.base ??
      historicalProbs.get(b.native_index)?.adjusted ??
      historicalProbs.get(b.native_index)?.base ??
      -1;
    return pb - pa;
  });
  for (const action of ordered) {
    if (!action.allowed) continue;
    const view = actionProbabilityView(action, analysis, outgoing);
    const row = el("div", "action" + (view.outgoing ? " historical" : ""));
    row.dataset.nativeIndex = String(action.native_index);
    const shown = view.nowAdj ?? view.nowBase;
    const shownNum = shown == null ? null : Number(shown);
    const bar = el("div", "probBar");
    const fill = el("span");
    fill.style.width = shownNum == null ? "0%" : `${Math.max(0, Math.min(100, shownNum * 100)).toFixed(1)}%`;
    bar.append(fill);
    const left = el("div", "actionMain");
    left.append(el("div", "actionName", displayActionLabel(action.label)));
    const bits = [];
    if (shown != null) bits.push(pct(shownNum));
    if (view.histBase != null) {
      bits.push(`was ${pct(Number(view.histAdj ?? view.histBase))}`);
      row.dataset.histBase = view.histBase;
    }
    if (view.histAdj != null) row.dataset.histAdj = view.histAdj;
    if (view.nowBase != null) row.dataset.currentBase = view.nowBase;
    if (view.nowAdj != null) row.dataset.currentAdj = view.nowAdj;
    if (view.outgoing) bits.push("taken");
    left.append(el("div", "meta", bits.join(" · ")));
    const button = el("button", null, "Play");
    button.type = "button";
    button.disabled = !canAct;
    button.addEventListener("click", () => onAct(action));
    row.append(bar, left, button);
    target.append(row);
  }
}

export function renderSummary(target, summary, markers) {
  target.replaceChildren();
  if (!summary) {
    target.append(el("p", "muted", "No incoming transition."));
    return;
  }
  target.append(el("div", null, displayActionLabel(summary.action || "")));
  if (summary.coverage && !String(summary.coverage).includes("aggregate_public_delta")) {
    target.append(el("p", "note", summary.coverage));
  }
  if (summary.enemy_phase_note) target.append(el("p", "note", summary.enemy_phase_note));
  if (summary.player?.net_hp !== undefined) {
    target.append(
      el("div", null, `Net HP ${fmt(summary.player.net_hp)} · block ${fmt(summary.player.net_block)} · energy ${fmt(summary.player.net_energy)}`)
    );
  }
  if (summary.powers && (Object.keys(summary.powers.added || {}).length || Object.keys(summary.powers.removed || {}).length)) {
    const added = Object.entries(summary.powers.added || {})
      .map(([key, amount]) => `${displayName(key)} ${fmt(amount)}`)
      .join(", ");
    const removed = Object.entries(summary.powers.removed || {})
      .map(([key, amount]) => `${displayName(key)} ${fmt(-amount)}`)
      .join(", ");
    target.append(el("div", "muted", `Powers: ${[added, removed].filter(Boolean).join(" · ") || "none"}`));
  }
  if (summary.enemies?.length) {
    for (const enemy of summary.enemies) {
      const name = monsterTitle({ content_key: enemy.identity?.content_key, slime_size: enemy.identity?.slime_size });
      const bits = [];
      if (enemy.net_hp != null) bits.push(`HP ${fmt(enemy.net_hp)}`);
      if (enemy.net_block != null) bits.push(`block ${fmt(enemy.net_block)}`);
      if (enemy.alive && enemy.alive.from !== enemy.alive.to) bits.push(`alive ${enemy.alive.from}→${enemy.alive.to}`);
      if (enemy.note) bits.push(enemy.note);
      target.append(el("div", "muted", `${name}: ${bits.join(" · ")}`));
    }
  }
  if (summary.error) target.append(el("div", "loss", summary.error));
  if (markers?.kills?.length) {
    target.append(el("div", "mark", "Kills: " + markers.kills.map((k) => displayName(k.content_key)).join(", ")));
  }
  if (markers?.uncertain_removals?.length) {
    target.append(el("div", "note", "Uncertain removals (not counted as kills)."));
  }
}

function fmt(value) {
  if (value === undefined || value === null) return "n/a";
  return (value > 0 ? "+" : "") + value;
}
