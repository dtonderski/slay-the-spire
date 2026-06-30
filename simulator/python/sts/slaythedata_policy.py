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
        floor = _parse_non_negative_floor(choice.get("floor"))
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


def guided_script_support_blocker(script: dict[str, Any]) -> dict[str, Any] | None:
    """Return a blocker for guided scripts that need unrecorded follow-up choices."""

    blockers = guided_script_support_audit(script)
    return blockers[0] if blockers else None


def guided_script_support_audit(script: dict[str, Any]) -> list[dict[str, Any]]:
    """Return every known guided-script support blocker.

    SlayTheData is a high-level run log.  Some screens need follow-up targets
    that the export does not record precisely enough for unattended replay.
    This audit is intentionally conservative: if a script needs an unrecorded
    grid target or multiple same-floor grid targets, guided collection should
    reject it before sending live commands.
    """

    blockers: list[dict[str, Any]] = []
    config = script.get("config") if isinstance(script.get("config"), dict) else {}
    bonus = str(config.get("neow_bonus") or "").upper()
    if bonus in _NEOW_GRID_TARGET_BONUSES:
        blockers.append(_blocked(
            "unsupported_neow_followup",
            f"Neow bonus {bonus} requires a card grid target that SlayTheData does not record",
        ) | {"category": "neow", "bonus": bonus})
    if bonus in _NEOW_CARD_REWARD_BONUSES and not _has_floor_zero_card_reward(script):
        blockers.append(_blocked(
            "missing_neow_card_reward",
            f"Neow bonus {bonus} requires a floor-0 card reward choice in the exported SlayTheData row",
        ) | {"category": "neow", "bonus": bonus})

    for decision in _list(script.get("floor_decisions")):
        if not isinstance(decision, dict):
            continue
        blockers.extend(_grid_support_blockers_for_floor(decision))
    return blockers


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

    decisionless_categories = {"boss_relic", "neow"}
    decision = None if category in decisionless_categories else floor_decision(script, floor)
    if category not in decisionless_categories and not decision:
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
        if _target_matches_label(target, label)
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


def identity_blocker(script: dict[str, Any], bridge_summary: dict[str, Any]) -> dict[str, Any] | None:
    """Return a blocker when visible live-run identity disagrees with the script."""

    config = script.get("config") if isinstance(script.get("config"), dict) else {}
    expected_character = _optional_string(config.get("character") or config.get("character_chosen"))
    observed_character = _optional_string(
        bridge_summary.get("class") or bridge_summary.get("character") or bridge_summary.get("character_chosen")
    )
    if expected_character and observed_character and expected_character.upper() != observed_character.upper():
        return _blocked(
            "run_identity_mismatch",
            f"script character {expected_character!r} does not match live character {observed_character!r}",
        )

    expected_ascension = _parse_int(
        config.get("ascension") if config.get("ascension") is not None else config.get("ascension_level")
    )
    observed_ascension = _parse_int(
        bridge_summary.get("ascension_level")
        if bridge_summary.get("ascension_level") is not None
        else bridge_summary.get("ascension")
    )
    if (
        expected_ascension is not None
        and observed_ascension is not None
        and expected_ascension != observed_ascension
    ):
        return _blocked(
            "run_identity_mismatch",
            f"script ascension {expected_ascension} does not match live ascension {observed_ascension}",
        )

    expected_seed = _optional_string(config.get("seed_played") or config.get("seed"))
    observed_seed = _optional_string(bridge_summary.get("seed") or bridge_summary.get("seed_played"))
    if expected_seed and observed_seed and expected_seed != observed_seed:
        return _blocked(
            "run_identity_mismatch",
            f"script seed {expected_seed!r} does not match live seed {observed_seed!r}",
        )
    return None


def match_map_choice(
    script: dict[str, Any],
    *,
    floor: int,
    choice_labels: list[str],
    next_nodes: list[dict[str, Any]] | None = None,
    map_nodes: list[dict[str, Any]] | None = None,
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
        disambiguated = _disambiguate_map_nodes_by_route(script, floor, node_matches, map_nodes)
        if len(disambiguated) == 1:
            slot = _node_choice_slot(disambiguated[0], choice_labels)
            if slot is not None:
                return {
                    "status": "matched",
                    "descriptor": {"kind": "ChooseVisibleOption", "option_slot": slot},
                    "target": target,
                    "matched_label": choice_labels[slot] if 0 <= slot < len(choice_labels) else str(disambiguated[0]),
                    "floor": floor,
                    "category": "map",
                    "ordinal": 0,
                    "match_evidence": "map_topology_lookahead",
                }
            return _blocked("target_not_visible", f"matched route {target!r} has no visible map choice slot")
        return _map_route_ambiguous(target, floor=floor, candidate_count=len(node_matches), source="next_nodes")

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
    return _map_route_ambiguous(target, floor=floor, candidate_count=len(label_matches), source="choice_labels")


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
    if category == "neow":
        return _neow_target_text(script, choice_labels)
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
    if category == "card_reward" and _normalized_token(target) == "skip":
        return {"kind": "SkipVisibleReward"}
    if category == "shop" and target == "__leave_shop__":
        return {"kind": "LeaveScreen"}
    return None


def _neow_target_text(script: dict[str, Any], choice_labels: list[str]) -> str | None:
    normalized_choices = {_normalized_token(label) for label in choice_labels}
    if "talk" in normalized_choices:
        return "talk"
    if "leave" in normalized_choices:
        return "leave"

    config = script.get("config") if isinstance(script.get("config"), dict) else {}
    bonus = _optional_string(config.get("neow_bonus"))
    cost = _optional_string(config.get("neow_cost"))
    bonus_text = _NEOW_BONUS_TEXT.get(str(bonus or "").upper())
    if not bonus_text:
        return None
    cost_text = _NEOW_COST_TEXT.get(str(cost or "NONE").upper())
    return f"{cost_text} {bonus_text}".strip() if cost_text else bonus_text


_NEOW_BONUS_TEXT = {
    "THREE_ENEMY_KILL": "enemies in your next three combats have 1 hp",
    "RANDOM_COLORLESS": "choose a colorless card to obtain",
    "RANDOM_COLORLESS_2": "choose a rare colorless card to obtain",
    "ONE_RANDOM_RARE_CARD": "obtain a random rare card",
    "RANDOM_RARE_CARD": "obtain a random rare card",
    "HUNDRED_GOLD": "obtain 100 gold",
    "TWO_FIFTY_GOLD": "obtain 250 gold",
    "TEN_PERCENT_HP_BONUS": "gain max hp",
    "TWENTY_PERCENT_HP_BONUS": "gain max hp",
    "THREE_RARE_CARDS": "choose a rare card to obtain",
    "THREE_CARDS": "choose a card to obtain",
    "REMOVE_CARD": "remove a card from your deck",
    "REMOVE_TWO": "remove 2 cards from your deck",
    "TRANSFORM_CARD": "transform a card",
    "TRANSFORM_TWO_CARDS": "transform 2 cards",
    "UPGRADE_CARD": "upgrade a card",
    "RANDOM_COMMON_RELIC": "obtain a random common relic",
    "ONE_RARE_RELIC": "obtain a random rare relic",
    "THREE_SMALL_POTIONS": "3 random potions",
    "BOSS_RELIC": "obtain a random boss relic",
}


_NEOW_COST_TEXT = {
    "NONE": "",
    "CURSE": "obtain a curse",
    "NO_GOLD": "lose all gold",
    "TEN_PERCENT_HP_LOSS": "lose max hp",
    "PERCENT_DAMAGE": "take damage",
}


_NEOW_CARD_REWARD_BONUSES = {
    "RANDOM_COLORLESS",
    "RANDOM_COLORLESS_2",
    "THREE_CARDS",
    "THREE_RARE_CARDS",
}


_NEOW_GRID_TARGET_BONUSES = {
    "REMOVE_CARD",
    "REMOVE_TWO",
    "TRANSFORM_CARD",
    "TRANSFORM_TWO_CARDS",
    "UPGRADE_CARD",
}


def _has_floor_zero_card_reward(script: dict[str, Any]) -> bool:
    decision = floor_decision(script, 0)
    if not decision:
        return False
    for entry in _list(decision.get("card_rewards")):
        if isinstance(entry, dict) and entry.get("picked"):
            return True
    return False


def _grid_support_blockers_for_floor(decision: dict[str, Any]) -> list[dict[str, Any]]:
    floor = decision.get("floor")
    targets: list[dict[str, Any]] = []

    for entry in _list(decision.get("campfires")):
        if not isinstance(entry, dict) or not entry.get("data"):
            continue
        targets.append(
            {
                "category": "campfire",
                "ordinal": entry.get("ordinal"),
                "kind": str(entry.get("key") or "campfire_grid").lower(),
                "target_count": 1,
            }
        )

    for entry in _list(decision.get("events")):
        if not isinstance(entry, dict):
            continue
        for key in ("cards_removed", "cards_upgraded", "cards_obtained"):
            values = [value for value in _list(entry.get(key)) if value]
            if not values:
                continue
            targets.append(
                {
                    "category": "event",
                    "ordinal": entry.get("ordinal"),
                    "kind": key,
                    "target_count": len(values),
                }
            )

    blockers: list[dict[str, Any]] = []
    for target in targets:
        if int(target["target_count"]) > 1:
            blockers.append(
                _blocked(
                    "unsupported_multi_card_grid",
                    f"floor {floor} {target['category']} {target['kind']} needs {target['target_count']} card targets",
                )
                | {"floor": floor, **target}
            )
    if len(targets) > 1:
        blockers.append(
            _blocked(
                "ambiguous_repeated_grid_floor",
                f"floor {floor} has {len(targets)} grid follow-up targets; SlayTheData does not order repeated grids precisely enough",
            )
            | {"floor": floor, "grid_target_count": len(targets)}
        )
    return blockers


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
    exact_relic = _first_visible_reward_identity(decision.get("relics_obtained"), "key", choice_labels)
    if exact_relic is not None:
        return exact_relic
    potions = decision.get("potions") if isinstance(decision.get("potions"), dict) else {}
    exact_potion = _first_visible_reward_identity(potions.get("obtained"), "key", choice_labels)
    if exact_potion is not None:
        return exact_potion
    if decision.get("relics_obtained") and "relic" in visible:
        return "relic"
    if potions.get("obtained") and "potion" in visible:
        return "potion"
    if decision.get("card_rewards") and "card" in visible:
        return "card"
    if "stolen_gold" in visible:
        return "stolen_gold"
    if "gold" in visible:
        return "gold"
    return None


def _first_visible_reward_identity(entries: Any, key: str, choice_labels: list[str]) -> str | None:
    for entry in _list(entries):
        if not isinstance(entry, dict):
            continue
        value = _optional_string(entry.get(key))
        if value and any(_target_matches_label(value, label) for label in choice_labels):
            return value
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


def _route_symbols_from_next_choice(script: dict[str, Any], floor: int, *, limit: int = 6) -> list[str]:
    path = ((script.get("route") or {}).get("path_per_floor") if isinstance(script.get("route"), dict) else None)
    if not isinstance(path, list):
        return []
    start_index = max(floor, 0)
    symbols: list[str] = []
    for value in path[start_index : start_index + limit]:
        symbol = _canonical_room_symbol(value)
        if symbol:
            symbols.append(symbol)
    return symbols


def _disambiguate_map_nodes_by_route(
    script: dict[str, Any],
    floor: int,
    node_matches: list[dict[str, Any]],
    map_nodes: list[dict[str, Any]] | None,
) -> list[dict[str, Any]]:
    symbols = _route_symbols_from_next_choice(script, floor)
    if len(symbols) < 2 or not map_nodes:
        return []
    graph = _map_graph_by_position(map_nodes)
    if not graph:
        return []
    disambiguated = []
    for node in node_matches:
        full_node = _map_node_for_visible_node(node, graph)
        if full_node is not None and _map_node_can_follow_symbols(full_node, symbols, graph):
            disambiguated.append(node)
    return disambiguated


def _map_graph_by_position(map_nodes: list[dict[str, Any]]) -> dict[tuple[int, int], dict[str, Any]]:
    graph: dict[tuple[int, int], dict[str, Any]] = {}
    for node in _list(map_nodes):
        if not isinstance(node, dict):
            continue
        x = _parse_int(node.get("x"))
        y = _parse_int(node.get("y"))
        if x is not None and y is not None:
            graph[(x, y)] = node
    return graph


def _map_node_for_visible_node(
    node: dict[str, Any],
    graph: dict[tuple[int, int], dict[str, Any]],
) -> dict[str, Any] | None:
    x = _parse_int(node.get("x"))
    y = _parse_int(node.get("y"))
    if x is None or y is None:
        return None
    return graph.get((x, y))


def _map_node_can_follow_symbols(
    node: dict[str, Any],
    symbols: list[str],
    graph: dict[tuple[int, int], dict[str, Any]],
) -> bool:
    if not symbols or not _room_symbol_matches(symbols[0], _node_room_symbol(node)):
        return False
    if len(symbols) == 1:
        return True
    children = _list(node.get("children"))
    for child in children:
        if not isinstance(child, dict):
            continue
        child_x = _parse_int(child.get("x"))
        child_y = _parse_int(child.get("y"))
        if child_x is None or child_y is None:
            continue
        child_node = graph.get((child_x, child_y))
        if child_node is not None and _map_node_can_follow_symbols(child_node, symbols[1:], graph):
            return True
    return False


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


def _map_route_ambiguous(
    target: str,
    *,
    floor: int,
    candidate_count: int,
    source: str,
) -> dict[str, Any]:
    return _blocked(
        "map_route_ambiguous",
        f"route {target!r} matched {candidate_count} map candidates via {source}",
    ) | {
        "floor": floor,
        "category": "map",
        "target": target,
        "candidate_count": candidate_count,
        "match_evidence": source,
    }


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


def _parse_non_negative_floor(value: Any) -> int | None:
    parsed = _parse_int(value)
    return parsed if parsed is not None and parsed >= 0 else None


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


def _normalized_token_without_digits(value: Any) -> str:
    return "".join(ch.lower() for ch in str(value) if ch.isalpha())


def _target_matches_label(target: Any, label: Any) -> bool:
    normalized_target = _normalized_token(target)
    normalized_label = _normalized_token(label)
    if normalized_target and normalized_target in normalized_label:
        return True
    digitless_target = _normalized_token_without_digits(target)
    digitless_label = _normalized_token_without_digits(label)
    return bool(digitless_target and digitless_target in digitless_label)
