"""SlayTheData run-level scripts for guided trace collection.

SlayTheData records high-level run history, not combat actions.  This module
keeps that boundary explicit: it converts exported raw runs into a floor-indexed
script and can match simple visible choices, while combat remains delegated to
the simulator search policy.
"""

from __future__ import annotations

from collections import defaultdict
import json
from pathlib import Path
from typing import Any


SCRIPT_SCHEMA_VERSION = 1


def build_guided_run_script(exported: dict[str, Any]) -> dict[str, Any]:
    """Build a normalized guided-collection script from one chunk-export row."""

    event = exported.get("event") if isinstance(exported.get("event"), dict) else exported
    source = {
        "kind": "slaythedata_chunk_export" if "event" in exported else "slaythedata_raw_run",
        "run_id": exported.get("run_id"),
        "play_id": event.get("play_id"),
        "source_file": exported.get("source_file"),
        "source_run_ordinal": exported.get("source_run_ordinal"),
    }
    floors: dict[int, dict[str, Any]] = defaultdict(_empty_floor_decision)

    for ordinal, choice in enumerate(_list(event.get("card_choices"))):
        floor = _parse_floor(choice.get("floor"))
        if floor is None:
            continue
        floors[floor]["card_rewards"].append(
            {
                "ordinal": ordinal,
                "picked": _clean_card_name(choice.get("picked")),
                "not_picked": [_clean_card_name(card) for card in _list(choice.get("not_picked"))],
                "skipped": bool(choice.get("picked") == "SKIP"),
            }
        )

    for ordinal, relic in enumerate(_list(event.get("relics_obtained"))):
        floor = _parse_floor(relic.get("floor") if isinstance(relic, dict) else None)
        if floor is None:
            continue
        floors[floor]["relics_obtained"].append(
            {
                "ordinal": ordinal,
                "key": str(relic.get("key")),
            }
        )

    for ordinal, event_choice in enumerate(_list(event.get("event_choices"))):
        floor = _parse_floor(event_choice.get("floor"))
        if floor is None:
            continue
        floors[floor]["events"].append(
            {
                "ordinal": ordinal,
                "event_name": _optional_string(event_choice.get("event_name")),
                "player_choice": _optional_string(event_choice.get("player_choice")),
                "damage_taken": _parse_int(event_choice.get("damage_taken")),
                "damage_healed": _parse_int(event_choice.get("damage_healed")),
                "max_hp_gain": _parse_int(event_choice.get("max_hp_gain")),
                "max_hp_loss": _parse_int(event_choice.get("max_hp_loss")),
                "gold_gain": _parse_int(event_choice.get("gold_gain")),
                "gold_loss": _parse_int(event_choice.get("gold_loss")),
                "cards_obtained": [_clean_card_name(card) for card in _list(event_choice.get("cards_obtained"))],
                "cards_removed": [_clean_card_name(card) for card in _list(event_choice.get("cards_removed"))],
                "cards_upgraded": [_clean_card_name(card) for card in _list(event_choice.get("cards_upgraded"))],
                "relics_obtained": [str(relic) for relic in _list(event_choice.get("relics_obtained"))],
            }
        )

    purchase_floors = _list(event.get("item_purchase_floors"))
    for ordinal, item in enumerate(_list(event.get("items_purchased"))):
        floor = _parse_floor(_value_at(purchase_floors, ordinal))
        if floor is None:
            continue
        floors[floor]["shop_purchases"].append(
            {
                "ordinal": ordinal,
                "item": str(item),
                "base_item": _clean_card_name(item),
            }
        )

    for ordinal, choice in enumerate(_list(event.get("campfire_choices"))):
        floor = _parse_floor(choice.get("floor"))
        if floor is None:
            continue
        floors[floor]["campfires"].append(
            {
                "ordinal": ordinal,
                "key": _optional_string(choice.get("key")),
                "data": _optional_string(choice.get("data")),
                "base_data": _clean_card_name(choice.get("data")) if choice.get("data") is not None else None,
            }
        )

    for ordinal, floor_value in enumerate(_list(event.get("potions_floor_usage"))):
        floor = _parse_floor(floor_value)
        if floor is not None:
            floors[floor]["potions"]["uses_allowed"] += 1
            floors[floor]["potions"]["usage_ordinals"].append(ordinal)

    for ordinal, potion in enumerate(_list(event.get("potions_obtained"))):
        floor = _parse_floor(potion.get("floor") if isinstance(potion, dict) else None)
        if floor is None:
            continue
        floors[floor]["potions"]["obtained"].append(
            {
                "ordinal": ordinal,
                "key": str(potion.get("key")),
            }
        )

    path_per_floor = _list(event.get("path_per_floor"))
    for floor, path_entry in enumerate(path_per_floor, start=1):
        floors[floor]["route"] = path_entry

    return {
        "schema": SCRIPT_SCHEMA_VERSION,
        "source": source,
        "config": {
            "character": event.get("character_chosen"),
            "ascension": _parse_int(event.get("ascension_level")),
            "build_version": event.get("build_version"),
            "seed_played": event.get("seed_played"),
            "seed_source_timestamp": event.get("seed_source_timestamp"),
            "special_seed": event.get("special_seed"),
            "neow_bonus": event.get("neow_bonus"),
            "neow_cost": event.get("neow_cost"),
        },
        "replay_policy": {
            "mode": "guided_collection",
            "exact_combat_actions": False,
            "on_illegal_high_level_choice": "discard_run",
            "on_legal_divergence": "continue_and_tag",
            "potion_budget_mode": "floor",
        },
        "route": {
            "path_taken": _list(event.get("path_taken")),
            "path_per_floor": path_per_floor,
        },
        "floor_decisions": [floors[floor] | {"floor": floor} for floor in sorted(floors)],
        "boss_relic_choices": _boss_relic_choices(event),
        "final_observed": {
            "floor_reached": _parse_int(event.get("floor_reached")),
            "victory": bool(event.get("victory")),
            "master_deck": [_clean_card_name(card) for card in _list(event.get("master_deck"))],
            "relics": [str(relic) for relic in _list(event.get("relics"))],
            "gold": _parse_int(event.get("gold")),
        },
        "replay_support": exported.get(
            "replay_support",
            {
                "run_level_choices": True,
                "exact_combat_actions": False,
                "potion_usage_has_floor_only": True,
            },
        ),
    }


def load_guided_run_script(path: str | Path, *, line_index: int = 0) -> dict[str, Any]:
    """Load one JSONL export row and convert it to a guided script."""

    rows = Path(path).read_text(encoding="utf-8").splitlines()
    if line_index < 0 or line_index >= len(rows):
        raise IndexError(f"line_index {line_index} is outside {len(rows)} exported rows")
    return build_guided_run_script(json.loads(rows[line_index]))


def floor_decision(script: dict[str, Any], floor: int) -> dict[str, Any] | None:
    for decision in _list(script.get("floor_decisions")):
        if decision.get("floor") == floor:
            return decision
    return None


def potion_uses_allowed_on_floor(script: dict[str, Any], floor: int) -> int:
    decision = floor_decision(script, floor)
    if not decision:
        return 0
    potions = decision.get("potions") if isinstance(decision.get("potions"), dict) else {}
    return int(potions.get("uses_allowed") or 0)


def match_visible_choice(
    script: dict[str, Any],
    *,
    floor: int,
    choice_labels: list[str],
    category: str,
    ordinal: int = 0,
    act: int | None = None,
) -> dict[str, Any]:
    """Match a guided run-level decision against visible CommunicationMod choices.

    This intentionally handles only screens where SlayTheData has a textual
    choice target.  It returns a blocker instead of guessing when matching is
    ambiguous or unsupported.
    """

    decision = None if category == "boss_relic" else floor_decision(script, floor)
    if category != "boss_relic" and not decision:
        return _blocked("missing_floor_decision", f"no SlayTheData decision for floor {floor}")

    target = _target_text_for_category(
        script,
        decision,
        category,
        ordinal,
        floor=floor,
        act=act,
        choice_labels=choice_labels,
    )
    if not target:
        return _blocked("missing_target", f"no {category} target at ordinal {ordinal} on floor {floor}")
    descriptor = _descriptor_for_target(category, target)
    if descriptor is not None:
        return {
            "status": "matched",
            "descriptor": descriptor,
            "target": target,
            "matched_label": target,
            "floor": floor,
            "category": category,
            "ordinal": ordinal,
        }

    matches = [
        index
        for index, label in enumerate(choice_labels)
        if _normalized_token(target) and _normalized_token(target) in _normalized_token(label)
    ]
    if len(matches) == 1:
        return {
            "status": "matched",
            "descriptor": {"kind": "ChooseVisibleOption", "option_slot": matches[0]},
            "target": target,
            "matched_label": choice_labels[matches[0]],
            "floor": floor,
            "category": category,
            "ordinal": ordinal,
        }
    if not matches:
        return _blocked("target_not_visible", f"{target!r} is not visible")
    return _blocked("ambiguous_target", f"{target!r} matched {len(matches)} visible choices")


def match_map_choice(
    script: dict[str, Any],
    *,
    floor: int,
    choice_labels: list[str],
    next_nodes: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    """Match the next SlayTheData path entry against visible map choices."""

    target = _route_target_for_next_choice(script, floor)
    if not target:
        return _blocked("missing_target", f"no map route target after floor {floor}")

    node_matches = [
        node
        for node in _list(next_nodes)
        if isinstance(node, dict) and _room_symbol_matches(target, _node_room_symbol(node))
    ]
    if len(node_matches) == 1:
        slot = _node_choice_slot(node_matches[0], choice_labels)
        if slot is not None:
            return {
                "status": "matched",
                "descriptor": {"kind": "ChooseVisibleOption", "option_slot": slot},
                "target": target,
                "matched_label": choice_labels[slot] if 0 <= slot < len(choice_labels) else str(node_matches[0]),
                "floor": floor,
                "category": "map",
                "ordinal": 0,
            }
        return _blocked("target_not_visible", f"matched route {target!r} has no visible map choice slot")
    if len(node_matches) > 1:
        return _blocked("ambiguous_target", f"route {target!r} matched {len(node_matches)} map nodes")

    label_matches = [
        index
        for index, label in enumerate(choice_labels)
        if _room_symbol_matches(target, label)
    ]
    if len(label_matches) == 1:
        return {
            "status": "matched",
            "descriptor": {"kind": "ChooseVisibleOption", "option_slot": label_matches[0]},
            "target": target,
            "matched_label": choice_labels[label_matches[0]],
            "floor": floor,
            "category": "map",
            "ordinal": 0,
        }
    if not label_matches:
        return _blocked("target_not_visible", f"route {target!r} is not visible")
    return _blocked("ambiguous_target", f"route {target!r} matched {len(label_matches)} visible choices")


def _target_text_for_category(
    script: dict[str, Any],
    decision: dict[str, Any] | None,
    category: str,
    ordinal: int,
    *,
    floor: int,
    act: int | None,
    choice_labels: list[str],
) -> str | None:
    if category == "card_reward":
        if decision is None:
            return None
        entry = _ordinal_entry(decision.get("card_rewards"), ordinal)
        return entry.get("picked") if entry else None
    if category == "event":
        if decision is None:
            return None
        entry = _ordinal_entry(decision.get("events"), ordinal)
        return entry.get("player_choice") if entry else None
    if category == "shop":
        if decision is None:
            return None
        entry = _ordinal_entry(decision.get("shop_purchases"), ordinal)
        if entry:
            return entry.get("item")
        purge_target = _shop_purge_target(decision, choice_labels)
        if purge_target is not None:
            return purge_target
        if _shop_leave_visible(choice_labels):
            return "__leave_shop__"
        return None
    if category == "reward":
        if decision is None:
            return None
        return _reward_target(decision, choice_labels)
    if category == "campfire":
        if decision is None:
            return None
        entry = _ordinal_entry(decision.get("campfires"), ordinal)
        if not entry:
            return None
        return entry.get("key") or entry.get("data")
    if category == "grid":
        if decision is None:
            return None
        campfire = _ordinal_entry(decision.get("campfires"), ordinal)
        if campfire and campfire.get("data"):
            return campfire.get("base_data") or campfire.get("data")
        event = _ordinal_entry(decision.get("events"), ordinal)
        if event:
            for key in ("cards_removed", "cards_upgraded", "cards_obtained"):
                values = _list(event.get(key))
                if values:
                    return str(values[0])
        return None
    if category == "boss_relic":
        entry = _boss_relic_choice(script, floor=floor, act=act, ordinal=ordinal)
        return entry.get("picked") if entry else None
    return None


def _descriptor_for_target(category: str, target: str) -> dict[str, Any] | None:
    if category == "shop" and target == "__leave_shop__":
        return {"kind": "LeaveScreen"}
    return None


def _shop_leave_visible(choice_labels: list[str]) -> bool:
    return any(_normalized_token(label) in {"leave", "proceed", "return"} for label in choice_labels)


def _shop_purge_target(decision: dict[str, Any], choice_labels: list[str]) -> str | None:
    if not any(event.get("cards_removed") for event in _list(decision.get("events")) if isinstance(event, dict)):
        return None
    tokens = {_normalized_token(label) for label in choice_labels}
    if "purge" in tokens:
        return "purge"
    if "remove" in tokens:
        return "remove"
    if "removecard" in tokens:
        return "remove card"
    return None


def _reward_target(decision: dict[str, Any], choice_labels: list[str]) -> str | None:
    visible = [_canonical_reward_label(label) for label in choice_labels]
    if decision.get("relics_obtained") and "relic" in visible:
        return "relic"
    potions = decision.get("potions") if isinstance(decision.get("potions"), dict) else {}
    if potions.get("obtained") and "potion" in visible:
        return "potion"
    if decision.get("card_rewards") and "card" in visible:
        return "card"
    if "stolen_gold" in visible:
        return "stolen_gold"
    if "gold" in visible:
        return "gold"
    return None


def _canonical_reward_label(value: Any) -> str:
    token = _normalized_token(value)
    if token in {"stolengold", "stolengoldreward"}:
        return "stolen_gold"
    if "relic" in token:
        return "relic"
    if "potion" in token:
        return "potion"
    if "card" in token:
        return "card"
    if "gold" in token:
        return "gold"
    return token


def _empty_floor_decision() -> dict[str, Any]:
    return {
        "route": None,
        "card_rewards": [],
        "relics_obtained": [],
        "events": [],
        "shop_purchases": [],
        "campfires": [],
        "potions": {
            "uses_allowed": 0,
            "usage_ordinals": [],
            "obtained": [],
        },
    }


def _boss_relic_choices(event: dict[str, Any]) -> list[dict[str, Any]]:
    rows = []
    for ordinal, choice in enumerate(_list(event.get("boss_relics"))):
        rows.append(
            {
                "act": ordinal + 1,
                "ordinal": ordinal,
                "picked": _optional_string(choice.get("picked")),
                "not_picked": [str(relic) for relic in _list(choice.get("not_picked"))],
            }
        )
    return rows


def _boss_relic_choice(
    script: dict[str, Any],
    *,
    floor: int,
    act: int | None,
    ordinal: int,
) -> dict[str, Any] | None:
    choices = [choice for choice in _list(script.get("boss_relic_choices")) if isinstance(choice, dict)]
    if not choices:
        return None
    target_act = act or _act_for_boss_relic_floor(floor)
    if target_act is not None:
        for choice in choices:
            if choice.get("act") == target_act:
                return choice
    return _ordinal_entry(choices, ordinal)


def _act_for_boss_relic_floor(floor: int) -> int | None:
    if floor <= 0:
        return None
    if floor <= 17:
        return 1
    if floor <= 34:
        return 2
    return 3


def _route_target_for_next_choice(script: dict[str, Any], floor: int) -> str | None:
    for candidate_floor in (floor + 1, floor):
        decision = floor_decision(script, candidate_floor)
        route = decision.get("route") if decision else None
        if route:
            return str(route)
    path = ((script.get("route") or {}).get("path_per_floor") if isinstance(script.get("route"), dict) else None)
    if isinstance(path, list):
        for index in (floor, floor - 1):
            if 0 <= index < len(path) and path[index]:
                return str(path[index])
    return None


def _node_room_symbol(node: dict[str, Any]) -> str | None:
    for key in ("symbol", "room_symbol", "roomSymbol", "room", "room_type", "roomType", "type"):
        value = node.get(key)
        if value is not None:
            return str(value)
    return None


def _node_choice_slot(node: dict[str, Any], choice_labels: list[str]) -> int | None:
    for key in ("choice_index", "choiceIndex", "slot", "index", "option_slot", "optionSlot"):
        parsed = _parse_int(node.get(key))
        if parsed is not None:
            return parsed
    x = _parse_int(node.get("x"))
    if x is not None:
        for index, label in enumerate(choice_labels):
            if f"x={x}" in str(label).lower():
                return index
        if 0 <= x < len(choice_labels):
            return x
    return 0 if len(choice_labels) == 1 else None


def _room_symbol_matches(target: Any, value: Any) -> bool:
    target_symbol = _canonical_room_symbol(target)
    value_symbol = _canonical_room_symbol(value)
    return bool(target_symbol and value_symbol and target_symbol == value_symbol)


def _canonical_room_symbol(value: Any) -> str:
    token = _normalized_token(value)
    aliases = {
        "m": "M",
        "monster": "M",
        "enemy": "M",
        "e": "E",
        "elite": "E",
        "?": "?",
        "event": "?",
        "unknown": "?",
        "$": "$",
        "shop": "$",
        "merchant": "$",
        "r": "R",
        "rest": "R",
        "campfire": "R",
        "t": "T",
        "treasure": "T",
        "chest": "T",
        "b": "B",
        "boss": "B",
    }
    raw = str(value).strip() if value is not None else ""
    if raw in aliases:
        return aliases[raw]
    return aliases.get(token, "")


def _ordinal_entry(entries: Any, ordinal: int) -> dict[str, Any] | None:
    candidates = [entry for entry in _list(entries) if isinstance(entry, dict)]
    for entry in candidates:
        if entry.get("ordinal") == ordinal:
            return entry
    if 0 <= ordinal < len(candidates):
        return candidates[ordinal]
    return None


def _blocked(reason: str, detail: str) -> dict[str, Any]:
    return {"status": "blocked", "reason": reason, "detail": detail}


def _list(value: Any) -> list[Any]:
    return value if isinstance(value, list) else []


def _value_at(values: list[Any], index: int) -> Any:
    return values[index] if 0 <= index < len(values) else None


def _parse_int(value: Any) -> int | None:
    if value is None or value == "":
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _parse_floor(value: Any) -> int | None:
    parsed = _parse_int(value)
    return parsed if parsed and parsed > 0 else None


def _optional_string(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def _clean_card_name(value: Any) -> str | None:
    text = _optional_string(value)
    if text is None:
        return None
    return text[:-1] if text.endswith("+") else text


def _normalized_token(value: Any) -> str:
    return "".join(ch.lower() for ch in str(value) if ch.isalnum())
