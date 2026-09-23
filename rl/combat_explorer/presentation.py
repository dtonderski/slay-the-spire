"""Display DTOs, action identity, and honest public-state summaries."""

from __future__ import annotations

from collections import Counter
from typing import Any

from combat_task import action_indices, combat_outcome
from sts_sim import Action, Observation

from combat_explorer.jsonutil import json_safe

SLOT_FIELDS = (
    "kind",
    "family",
    "hand_slot",
    "potion_slot",
    "option_slot",
    "target_slot",
    "card_slot",
    "node_slot",
    "reward_slot",
    "shop_slot",
)

MARKER_LIMITATIONS = (
    "Kill markers require an unambiguous alive→dead transition for the same slot identity. "
    "Escapes are excluded. Missing, replaced, or split enemies are marked as uncertain removals, "
    "not kills. Individual enemy actions are not available from the fair decision API; summaries "
    "are net public-state deltas."
)


def action_descriptor(action: Action) -> dict[str, Any]:
    return {name: getattr(action, name) for name in SLOT_FIELDS}


def descriptors_match(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return action_descriptor_from_mapping(left) == action_descriptor_from_mapping(right)


def action_descriptor_from_mapping(value: dict[str, Any]) -> dict[str, Any]:
    return {name: value.get(name) for name in SLOT_FIELDS}


def action_manifest(actions: tuple[Action, ...] | list[Action]) -> list[dict[str, Any]]:
    return [action_descriptor(action) for action in actions]


def _card_name(card: object) -> str:
    key = str(getattr(card, "content_key"))
    upgrades = int(getattr(card, "upgrade_level", 0) or 0)
    if upgrades == 1:
        return f"{key}+"
    if upgrades > 1:
        return f"{key}+{upgrades}"
    return key


def _hand_card(observation: Observation, slot: int | None) -> object | None:
    if observation.kind != "combat" or slot is None:
        return None
    for entry in observation.screen.hand:
        if entry.slot == slot:
            return entry.card
    return None


def _potion_name(observation: Observation, slot: int | None) -> str:
    if slot is None:
        return "potion"
    potions = observation.context.potion_slots
    if 0 <= slot < len(potions):
        key = potions[slot].content_key
        return "empty slot" if key is None else str(key)
    return f"potion slot {slot}"


def _monster_name(observation: Observation, slot: int | None) -> str:
    if observation.kind != "combat" or slot is None:
        return f"target {slot}"
    for monster in observation.screen.monsters:
        if monster.slot == slot:
            size = f" {monster.slime_size}" if monster.slime_size else ""
            return f"{monster.content_key}{size}#{monster.slot}"
    return f"target {slot}"


def _option_name(observation: Observation, slot: int | None) -> str:
    if observation.kind != "combat" or observation.screen.selection is None or slot is None:
        return f"option {slot}"
    for option in observation.screen.selection.options:
        if option.slot == slot:
            return _card_name(option.card)
    return f"option {slot}"


def action_label(observation: Observation, action: Action) -> str:
    kind = action.kind
    if kind == "play_hand_slot":
        card = _hand_card(observation, action.hand_slot)
        name = _card_name(card) if card is not None else f"hand {action.hand_slot}"
        if action.target_slot is not None:
            return f"Play {name} -> {_monster_name(observation, action.target_slot)}"
        return f"Play {name}"
    if kind == "use_potion_slot":
        name = _potion_name(observation, action.potion_slot)
        if action.target_slot is not None:
            return f"Use {name} -> {_monster_name(observation, action.target_slot)}"
        return f"Use {name}"
    if kind == "discard_potion_slot":
        return f"Discard {_potion_name(observation, action.potion_slot)}"
    if kind == "end_turn":
        return "End turn"
    if kind == "toggle_visible_card":
        return f"Toggle {_option_name(observation, action.option_slot)}"
    if kind == "choose_visible_option":
        return f"Choose {_option_name(observation, action.option_slot)}"
    if kind == "confirm_selection":
        return "Confirm selection"
    if kind == "confirm_selection_without_retrieval":
        return "Confirm without retrieval"
    if kind == "skip_selection":
        return "Skip selection"
    slots = [
        f"{name}={getattr(action, name)}"
        for name in SLOT_FIELDS
        if name not in ("kind", "family") and getattr(action, name) is not None
    ]
    extra = f" ({', '.join(slots)})" if slots else ""
    return f"{kind}{extra}"


def present_card(card: object) -> dict[str, Any]:
    return {
        "content_key": str(getattr(card, "content_key")),
        "name": _card_name(card),
        "cost": getattr(card, "cost", None),
        "cost_is_modified": getattr(card, "cost_is_modified", False),
        "upgrade_level": getattr(card, "upgrade_level", 0),
        "bottled": getattr(card, "bottled", False),
        "temporary": getattr(card, "temporary", False),
    }


def _present_powers(powers: object) -> list[dict[str, Any]]:
    return [{"key": str(power.key), "amount": int(power.amount)} for power in powers]  # type: ignore[not-iterable]


def _present_intent(intent: object) -> dict[str, Any]:
    visibility = str(getattr(intent, "visibility"))
    payload: dict[str, Any] = {"visibility": visibility}
    if visibility == "visible":
        payload["category"] = str(getattr(intent, "category"))
        payload["damage"] = getattr(intent, "damage", None)
        payload["hits"] = getattr(intent, "hits", None)
    return payload


def _present_monster(monster: object) -> dict[str, Any]:
    size = getattr(monster, "slime_size", None)
    return {
        "slot": int(monster.slot),  # type: ignore[attr-defined]
        "content_key": str(monster.content_key),  # type: ignore[attr-defined]
        "slime_size": None if size is None else str(size),
        "hp": int(monster.hp),  # type: ignore[attr-defined]
        "max_hp": int(monster.max_hp),  # type: ignore[attr-defined]
        "block": int(monster.block),  # type: ignore[attr-defined]
        "powers": _present_powers(monster.powers),  # type: ignore[attr-defined]
        "intent": _present_intent(monster.intent),  # type: ignore[attr-defined]
        "alive": bool(monster.alive),  # type: ignore[attr-defined]
        "escaped": bool(monster.escaped),  # type: ignore[attr-defined]
        "minion": bool(monster.minion),  # type: ignore[attr-defined]
        "targetable": bool(monster.targetable),  # type: ignore[attr-defined]
    }


def _present_pile(pile: object, *, unordered: bool) -> dict[str, Any]:
    cards = list(getattr(pile, "cards"))
    known = [
        {"position": int(entry.position), "card": present_card(entry.card)}
        for entry in getattr(pile, "known_positions")
    ]
    identities = [present_card(card) for card in cards]
    if unordered:
        identities = sorted(identities, key=lambda card: (card["content_key"], card["upgrade_level"], card["name"]))
    return {
        "count": len(cards),
        "known_positions": known,
        "identities": identities,
        "order": "unknown" if unordered else "listed",
    }


def present_context(observation: Observation) -> dict[str, Any]:
    context = observation.context
    return {
        "ascension": context.ascension,
        "act": context.act,
        "floor": context.floor,
        "gold": context.gold,
        "player_hp": context.player_hp,
        "player_max_hp": context.player_max_hp,
        "deck_size": len(context.deck),
        "relics": [{"slot": relic.slot, "content_key": str(relic.content_key)} for relic in context.relics],
        "potions": [
            {"slot": potion.slot, "content_key": None if potion.content_key is None else str(potion.content_key)}
            for potion in context.potion_slots
        ],
    }


def public_hp(observation: object) -> dict[str, int]:
    if getattr(observation, "kind", None) == "combat":
        try:
            player = observation.screen.player  # type: ignore[attr-defined]
            return {"hp": int(player.hp), "max_hp": int(player.max_hp)}
        except Exception:
            pass
    context = getattr(observation, "context", None)
    if context is None:
        return {"hp": 0, "max_hp": 0}
    return {"hp": int(getattr(context, "player_hp", 0)), "max_hp": int(getattr(context, "player_max_hp", 0))}


def present_board(observation: Observation) -> dict[str, Any]:
    outcome = _outcome_name(observation)
    hp = public_hp(observation)
    board: dict[str, Any] = {
        "kind": observation.kind,
        "phase": observation.phase,
        "context": present_context(observation),
        "outcome": outcome,
        "hp": hp["hp"],
        "max_hp": hp["max_hp"],
        "combat": None,
        "selection": None,
        "terminal_screen": None,
        "note": None,
    }
    if observation.kind == "combat":
        screen = observation.screen
        player = screen.player
        board["combat"] = {
            "phase": screen.phase,
            "player": {
                "hp": player.hp,
                "max_hp": player.max_hp,
                "block": player.block,
                "energy": player.energy,
                "max_energy": player.max_energy,
                "powers": _present_powers(player.powers),
            },
            "hand": [{"slot": entry.slot, "card": present_card(entry.card)} for entry in screen.hand],
            "monsters": [_present_monster(monster) for monster in screen.monsters],
            "draw_pile": _present_pile(screen.draw_pile, unordered=True),
            "discard_pile": _present_pile(screen.discard_pile, unordered=False),
            "exhaust_pile": _present_pile(screen.exhaust_pile, unordered=False),
            "orbs": [
                {"slot": orb.slot, "orb": None if orb.orb is None else json_safe(orb.orb)} for orb in screen.orb_slots
            ],
            "public_counters": [{"key": str(counter.key), "value": int(counter.value)} for counter in screen.public_counters],
        }
        if screen.selection is not None:
            board["selection"] = {
                "kind": str(screen.selection.kind),
                "selected_slots": list(screen.selection.selected_slots),
                "options": [
                    {"slot": option.slot, "card": present_card(option.card)} for option in screen.selection.options
                ],
            }
        return board
    board["note"] = (
        "This node is a post-combat public screen, not a live combat board. "
        "Do not treat it as the preceding combat state's current view."
    )
    board["terminal_screen"] = {"kind": observation.kind, "phase": observation.phase}
    return board


def _outcome_name(observation: Observation) -> str | None:
    try:
        won = combat_outcome(observation)
    except RuntimeError:
        return "unexpected"
    if won is True:
        return "win"
    if won is False:
        return "loss"
    return None


def present_actions(observation: Observation, actions: tuple[Action, ...] | list[Action]) -> list[dict[str, Any]]:
    allowed = set(action_indices_safe(observation, actions))
    rows = []
    for index, action in enumerate(actions):
        rows.append(
            {
                "native_index": index,
                "descriptor": action_descriptor(action),
                "label": action_label(observation, action),
                "allowed": index in allowed,
            }
        )
    return rows


def action_indices_safe(observation: Observation, actions: tuple[Action, ...] | list[Action]) -> list[int]:
    from sts_sim import Decision

    decision = Decision(schema_version=0, revision=0, observation=observation, actions=tuple(actions))
    return action_indices(decision)


def _monster_identity(monster: object) -> tuple[int, str, str | None]:
    size = getattr(monster, "slime_size", None)
    return (int(monster.slot), str(monster.content_key), None if size is None else str(size))  # type: ignore[attr-defined]


def _power_counter(powers: object) -> Counter[str]:
    counts: Counter[str] = Counter()
    for power in powers:  # type: ignore[not-iterable]
        counts[str(power.key)] += int(power.amount)
    return counts


def _card_counter(cards: object) -> Counter[tuple[str, int]]:
    counts: Counter[tuple[str, int]] = Counter()
    for card in cards:  # type: ignore[not-iterable]
        counts[(str(card.content_key), int(getattr(card, "upgrade_level", 0) or 0))] += 1
    return counts


def summarize_transition(parent: Observation, child: Observation, action: Action) -> dict[str, Any]:
    """Net public deltas only; not a chronological enemy-action log."""
    label = action_label(parent, action)
    summary: dict[str, Any] = {
        "action": label,
        "kind": action.kind,
        "end_turn_submitted": action.kind == "end_turn",
        "coverage": "aggregate_public_delta; individual enemy actions unavailable",
        "player": {},
        "piles": {},
        "powers": {},
        "hand": {},
        "enemies": [],
        "outcome": _outcome_name(child),
        "enemy_phase_note": None,
    }
    if action.kind == "end_turn":
        summary["enemy_phase_note"] = (
            "End turn was accepted. An enemy phase may have resolved before the next decision. "
            "Previous visible intents are not proof of the actions that executed."
        )
    parent_hp = parent.context.player_hp
    child_hp = child.context.player_hp
    summary["player"]["net_hp"] = child_hp - parent_hp
    summary["player"]["hp"] = {"from": parent_hp, "to": child_hp}
    if parent.kind == "combat" and child.kind == "combat":
        pp, cp = parent.screen.player, child.screen.player
        summary["player"]["net_block"] = cp.block - pp.block
        summary["player"]["net_energy"] = cp.energy - pp.energy
        summary["player"]["block"] = {"from": pp.block, "to": cp.block}
        summary["player"]["energy"] = {"from": pp.energy, "to": cp.energy}
        summary["piles"] = {
            "hand_count": {"from": len(parent.screen.hand), "to": len(child.screen.hand)},
            "draw_count": {"from": len(parent.screen.draw_pile.cards), "to": len(child.screen.draw_pile.cards)},
            "discard_count": {
                "from": len(parent.screen.discard_pile.cards),
                "to": len(child.screen.discard_pile.cards),
            },
            "exhaust_count": {
                "from": len(parent.screen.exhaust_pile.cards),
                "to": len(child.screen.exhaust_pile.cards),
            },
        }
        parent_hand = _card_counter(entry.card for entry in parent.screen.hand)
        child_hand = _card_counter(entry.card for entry in child.screen.hand)
        summary["hand"] = {
            "added": [
                {"content_key": key, "upgrade_level": upgrade, "count": count}
                for (key, upgrade), count in (child_hand - parent_hand).items()
            ],
            "removed": [
                {"content_key": key, "upgrade_level": upgrade, "count": count}
                for (key, upgrade), count in (parent_hand - child_hand).items()
            ],
        }
        parent_powers = _power_counter(pp.powers)
        child_powers = _power_counter(cp.powers)
        summary["powers"] = {
            "added": dict(child_powers - parent_powers),
            "removed": dict(parent_powers - child_powers),
        }
        summary["enemies"] = _enemy_deltas(parent, child)
    elif parent.kind == "combat":
        summary["player"]["note"] = "Child is no longer a combat screen; remaining combat fields are unavailable."
        summary["enemies"] = _enemy_deltas(parent, child)
    return summary


def _enemy_deltas(parent: Observation, child: Observation) -> list[dict[str, Any]]:
    if parent.kind != "combat":
        return []
    parent_map = {_monster_identity(monster): monster for monster in parent.screen.monsters}
    child_map: dict[tuple[int, str, str | None], Any] = {}
    if child.kind == "combat":
        child_map = {_monster_identity(monster): monster for monster in child.screen.monsters}
    rows: list[dict[str, Any]] = []
    for identity, monster in parent_map.items():
        current = child_map.get(identity)
        row: dict[str, Any] = {
            "identity": {"slot": identity[0], "content_key": identity[1], "slime_size": identity[2]},
            "alive": {"from": bool(monster.alive), "to": None if current is None else bool(current.alive)},
            "escaped": {"from": bool(monster.escaped), "to": None if current is None else bool(current.escaped)},
        }
        if current is not None:
            row["net_hp"] = int(current.hp) - int(monster.hp)
            row["net_block"] = int(current.block) - int(monster.block)
        else:
            row["net_hp"] = None
            row["note"] = "Enemy identity was not present on the child combat screen."
        rows.append(row)
    if child.kind == "combat":
        for identity, monster in child_map.items():
            if identity not in parent_map:
                rows.append(
                    {
                        "identity": {"slot": identity[0], "content_key": identity[1], "slime_size": identity[2]},
                        "alive": {"from": None, "to": bool(monster.alive)},
                        "escaped": {"from": None, "to": bool(monster.escaped)},
                        "note": "New enemy identity on the child screen.",
                    }
                )
    return rows


def transition_markers(parent: Observation, child: Observation, action: Action) -> dict[str, Any]:
    kills: list[dict[str, Any]] = []
    uncertain: list[dict[str, Any]] = []
    escaped: list[dict[str, Any]] = []
    if parent.kind == "combat" and child.kind == "combat":
        child_by_slot = {monster.slot: monster for monster in child.screen.monsters}
        child_identities = {_monster_identity(monster) for monster in child.screen.monsters}
        for monster in parent.screen.monsters:
            if not monster.alive:
                continue
            identity = _monster_identity(monster)
            current = next((item for item in child.screen.monsters if _monster_identity(item) == identity), None)
            payload = {"slot": monster.slot, "content_key": str(monster.content_key), "slime_size": identity[2]}
            if current is not None:
                if current.escaped and not monster.escaped:
                    escaped.append(payload)
                elif not current.alive and not current.escaped:
                    kills.append(payload)
                continue
            same_slot = child_by_slot.get(monster.slot)
            if same_slot is not None and _monster_identity(same_slot) != identity:
                uncertain.append({**payload, "reason": "slot_reused_with_different_identity"})
            elif identity not in child_identities:
                if same_slot is None:
                    uncertain.append({**payload, "reason": "missing_from_child_screen"})
                else:
                    uncertain.append({**payload, "reason": "identity_unmatched"})
    elif parent.kind == "combat" and child.kind != "combat":
        uncertain.append({"reason": "left_combat_screen", "note": "Individual enemy deaths were not confirmed."})
    return {
        "turn_end": action.kind == "end_turn",
        "kills": kills,
        "uncertain_removals": uncertain,
        "escaped": escaped,
        "combat_win": _outcome_name(child) == "win",
        "combat_loss": _outcome_name(child) == "loss",
        "limitations": MARKER_LIMITATIONS,
    }


def fallback_summary(action_kind: str, label: str, detail: str) -> dict[str, Any]:
    return {
        "action": label,
        "kind": action_kind,
        "coverage": "summary_unavailable",
        "error": detail,
    }


def fallback_markers() -> dict[str, Any]:
    return {
        "turn_end": False,
        "kills": [],
        "uncertain_removals": [],
        "escaped": [],
        "combat_win": False,
        "combat_loss": False,
        "limitations": MARKER_LIMITATIONS,
        "error": "marker_unavailable",
    }
