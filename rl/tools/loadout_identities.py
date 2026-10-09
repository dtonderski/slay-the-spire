"""Explicit run-log identity translation; unknown IDs fail closed, never fuzzy-match.

Punctuation/case folding follows the simulator's relic name normalization.
Remaining aliases are listed explicitly so historical game IDs are auditable.
Ghostly/Apparition is also recognized by sts_verify::sim_real's card importer.
This checks the public catalog, not full mechanical support for every relic.
"""

import ast
import hashlib
import re
from pathlib import Path

CATALOG = Path(__file__).resolve().parents[2] / "simulator/python/sts_sim/content_ids.py"
CATALOG_SHA256 = hashlib.sha256(CATALOG.read_bytes()).hexdigest()


def normalized(value: str) -> str:
    return re.sub(r"[^a-z0-9]", "", value.lower())


def catalog(name: str) -> dict[str, str]:
    result = {}
    for node in ast.parse(CATALOG.read_text()).body:
        if isinstance(node, ast.ClassDef) and node.name == name:
            for entry in node.body:
                if isinstance(entry, ast.Assign) and isinstance(entry.value, ast.Constant):
                    value = entry.value.value
                    if isinstance(value, str) and not value.endswith("+"):
                        key = normalized(value)
                        if key in result and result[key] != value:
                            raise ValueError(f"Ambiguous normalized identity: {value}")
                        result[key] = value
    if not result:
        raise ValueError(f"No {name} catalog")
    return result


CARDS = catalog("CardKey")
RELICS = catalog("RelicKey")
POTIONS = catalog("PotionKey")
CARD_ALIASES = {"Ghostly": "Apparition"}
RELIC_ALIASES = {
    "NeowsBlessing": "Neow's Lament",
    "Boot": "The Boot",
    "Paper Frog": "Paper Phrog",
    "Molten Egg 2": "Molten Egg",
    "Toxic Egg 2": "Toxic Egg",
    "Frozen Egg 2": "Frozen Egg",
    "WingedGreaves": "Wing Boots",
    "Sling": "Sling of Courage",
    "CultistMask": "Cultist Headpiece",
}
POTION_ALIASES = {
    "Ancient Potion": "ancient",
    "AttackPotion": "attack",
    "Block Potion": "block",
    "BloodPotion": "blood",
    "ColorlessPotion": "colorless",
    "CultistPotion": "cultist",
    "Dexterity Potion": "dexterity",
    "DuplicationPotion": "duplication",
    "ElixirPotion": "elixir",
    "Energy Potion": "energy",
    "Explosive Potion": "explosive",
    "FairyPotion": "fairy_in_a_bottle",
    "FearPotion": "fear",
    "Fire Potion": "fire",
    "PowerPotion": "power",
    "Regen Potion": "regen",
    "SkillPotion": "skill",
    "SpeedPotion": "speed",
    "SteroidPotion": "flex",
    "Strength Potion": "strength",
    "Swift Potion": "swift",
    "Weak Potion": "weak",
}


def resolve(value: str, kind: str) -> str:
    tables, aliases = {
        "card": (CARDS, CARD_ALIASES),
        "relic": (RELICS, RELIC_ALIASES),
        "potion": (POTIONS, POTION_ALIASES),
    }[kind]
    key = normalized(aliases.get(value, value))
    if key not in tables:
        raise ValueError(f"unmapped_{kind}:{value}")
    return tables[key]
