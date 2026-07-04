const runInfo = document.querySelector("#runInfo");
const choices = document.querySelector("#choices");
const hand = document.querySelector("#hand");
const monsters = document.querySelector("#monsters");
const raw = document.querySelector("#raw");
const tracePath = document.querySelector("#tracePath");
const commandInput = document.querySelector("#commandInput");
const sendCommand = document.querySelector("#sendCommand");
const characterSelect = document.querySelector("#characterSelect");
const ascensionInput = document.querySelector("#ascensionInput");
const seedInput = document.querySelector("#seedInput");
const startRun = document.querySelector("#startRun");

let latest = null;

async function api(path, options) {
  const response = await fetch(path, options);
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return response.json();
}

async function send(command) {
  await api("/api/command", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ command }),
  });
  commandInput.value = "";
  await refresh();
}

function clear(element) {
  element.replaceChildren();
}

function button(label, command, title) {
  const item = document.createElement("button");
  item.textContent = label;
  item.title = title || command;
  item.addEventListener("click", () => send(command).catch(showError));
  return item;
}

function field(label, value) {
  const dt = document.createElement("dt");
  dt.textContent = label;
  const dd = document.createElement("dd");
  dd.textContent = value ?? "";
  runInfo.append(dt, dd);
}

function renderRun(summary, status) {
  clear(runInfo);
  field("Step", status?.step ?? summary?.step);
  field("Status", status?.status);
  field("Screen", summary?.screen_type);
  field("Room", summary?.room_type);
  field("Floor", summary?.floor);
  field("HP", `${summary?.current_hp ?? ""}/${summary?.max_hp ?? ""}`);
  field("Gold", summary?.gold);
  field("Seed", summary?.seed);
  field("Class", summary?.class);
  if (summary?.error) field("Error", summary.error);
  tracePath.textContent = status?.trace_path || "No trace loaded";
}

function renderChoices(summary) {
  clear(choices);
  (summary?.choices || []).forEach((choice, index) => {
    choices.append(button(`${index}: ${choice}`, `CHOOSE ${index}`));
  });
}

function renderHand(summary) {
  clear(hand);
  const combat = summary?.combat;
  if (!combat?.hand) return;
  const liveMonsters = (combat.monsters || []).filter((monster) => !monster.gone && !monster.half_dead);
  combat.hand.forEach((card) => {
    const group = document.createElement("div");
    group.className = "cardCommand";
    const label = document.createElement("div");
    label.className = "cardLabel";
    label.textContent = `${card.index}. ${card.name} (${card.cost})`;
    group.append(label);

    if (card.has_target) {
      liveMonsters.forEach((monster) => {
        const item = button(`-> ${monster.index}: ${monster.name}`, `PLAY ${card.index} ${monster.index}`);
        item.disabled = !card.playable;
        group.append(item);
      });
    } else {
      const item = button("Play", `PLAY ${card.index}`);
      item.disabled = !card.playable;
      group.append(item);
    }

    hand.append(group);
  });
}

function renderMonsters(summary) {
  clear(monsters);
  const combat = summary?.combat;
  if (!combat?.monsters) return;
  combat.monsters.forEach((monster) => {
    const row = document.createElement("div");
    row.className = "monster";
    row.textContent = `${monster.index}: ${monster.name} ${monster.hp}/${monster.max_hp} ${monster.intent || ""}`;
    monsters.append(row);
  });
}

function renderPotions(state) {
  const game = state?.message?.game_state;
  const potions = game?.potions || [];
  if (!potions.length) return;

  const article = document.querySelector("#potionsArticle");
  const list = document.querySelector("#potions");
  if (!article || !list) return;

  article.hidden = false;
  clear(list);

  const liveMonsters = (game.combat_state?.monsters || [])
    .map((monster, index) => ({ ...monster, index }))
    .filter((monster) => !monster.is_gone && !monster.half_dead);

  potions.forEach((potion, index) => {
    if (!potion.can_use || potion.name === "Potion Slot") return;
    const slot = index;
    const group = document.createElement("div");
    group.className = "cardCommand";
    const label = document.createElement("div");
    label.className = "cardLabel";
    label.textContent = `${slot}. ${potion.name}`;
    group.append(label);

    if (potion.requires_target) {
      liveMonsters.forEach((monster) => {
        group.append(button(`-> ${monster.index}: ${monster.name}`, `POTION USE ${slot} ${monster.index}`));
      });
    } else {
      group.append(button("Use", `POTION USE ${slot}`));
    }
    list.append(group);
  });
}

function render(data) {
  latest = data;
  const { summary, status } = data;
  renderRun(summary, status);
  renderChoices(summary);
  renderHand(summary);
  renderMonsters(summary);
  renderPotions(data.state);
  raw.textContent = JSON.stringify(data, null, 2);
}

function showError(error) {
  raw.textContent = String(error?.stack || error);
}

async function refresh() {
  render(await api("/api/session"));
}

document.querySelectorAll("[data-command]").forEach((item) => {
  item.addEventListener("click", () => send(item.dataset.command).catch(showError));
});

sendCommand.addEventListener("click", () => {
  const command = commandInput.value.trim();
  if (command) send(command).catch(showError);
});

startRun.addEventListener("click", () => {
  const character = characterSelect.value;
  const ascension = ascensionInput.value || "0";
  const seed = seedInput.value.trim();
  if (!seed) {
    showError(new Error("Seed is required, e.g. CODEX07"));
    return;
  }
  send(`START ${character} ${ascension} ${seed}`).catch(showError);
});

commandInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    sendCommand.click();
  }
});

setInterval(() => refresh().catch(showError), 1000);
refresh().catch(showError);
