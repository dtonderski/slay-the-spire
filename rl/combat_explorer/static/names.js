const KEEP_TOKENS = new Set([
  "Play",
  "Use",
  "Discard",
  "End",
  "Select",
  "Skip",
  "Confirm",
  "turn",
  "from",
  "to",
  "and",
  "Root",
  "root",
]);

export function displayName(raw) {
  if (raw == null) return "";
  const original = String(raw).trim();
  if (!original) return "";
  if (original === "empty" || original === "empty slot") return "Empty";
  let text = original.replace(/_(R|G|B|P)$/u, "");
  text = text.replaceAll("_", " ");
  const letters = text.replace(/[^A-Za-z]/g, "");
  if (letters && (letters === letters.toUpperCase() || letters === letters.toLowerCase())) {
    text = text.toLowerCase().replace(/(^|[\s/+-])([a-z])/g, (_, prefix, letter) => prefix + letter.toUpperCase());
  }
  return text;
}

export function displayActionLabel(label) {
  if (!label) return "Root";
  if (label === "root") return "Root";
  return String(label)
    .replace(/#\d+\b/g, "")
    .replace(/[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)*/g, (token) => (KEEP_TOKENS.has(token) ? token : displayName(token)))
    .replace(/\s+/g, " ")
    .replace(/\s+→\s+/g, " → ")
    .trim();
}

export function monsterTitle(monster) {
  if (!monster) return "Enemy";
  const size = monster.slime_size ? ` (${monster.slime_size})` : "";
  return `${displayName(monster.content_key)}${size}`;
}
