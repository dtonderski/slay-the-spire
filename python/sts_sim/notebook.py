"""Plain-text helpers for inspecting the unified run API in a notebook.

The formatter only consumes fair observations and public actions. It does not
inspect native state, snapshots, internal IDs, or RNG state. The output is
ordinary text so it works the same in Jupyter, a terminal, and a log file.
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Iterable
from typing import cast

from .fair import (
    FairCard,
    FairCombatObservation,
    FairCounter,
    FairMonster,
    FairPower,
    FairRunObservation,
)
from .run import Action, Decision

__all__ = [
    "action_label",
    "format_action",
    "format_actions",
    "format_decision",
    "format_observation",
    "show_actions",
    "show_decision",
]


def _humanize(value: object) -> str:
    return str(value).replace("_", " ").strip()


def _name(value: object) -> str:
    return _humanize(value).title()


def _records(screen: dict[str, object], key: str) -> tuple[dict[str, object], ...]:
    value = screen.get(key)
    if not isinstance(value, list):
        return ()
    return tuple(cast(dict[str, object], item) for item in value if isinstance(item, dict))


def _integers(value: object) -> tuple[int, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, int))


def _integer_list(screen: dict[str, object], key: str) -> tuple[int, ...]:
    return _integers(screen.get(key))


def _strings(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, str))


def _card_name(card: FairCard) -> str:
    name = _name(card.content_key)
    if card.upgrade_level:
        name += f"+{card.upgrade_level}"
    return name


def _card_text(card: FairCard, *, include_cost: bool = True) -> str:
    details: list[str] = []
    if include_cost:
        details.append("cost X" if card.cost < 0 else f"cost {card.cost}")
    if card.cost_is_modified:
        details.append("modified cost")
    if card.cost_resets_next_turn:
        details.append("resets next turn")
    if card.bottled:
        details.append("bottled")
    if card.temporary:
        details.append("temporary")
    if card.dynamic.rampage_damage_bonus is not None:
        details.append(f"Rampage +{card.dynamic.rampage_damage_bonus}")
    if card.dynamic.ritual_dagger_damage_bonus is not None:
        details.append(f"Ritual Dagger +{card.dynamic.ritual_dagger_damage_bonus}")
    if card.dynamic.windmill_retain_damage is not None:
        details.append(f"Windmill +{card.dynamic.windmill_retain_damage}")
    return f"{_card_name(card)} ({', '.join(details)})" if details else _card_name(card)


def _card_record_name(record: dict[str, object]) -> str:
    card = record.get("card")
    if isinstance(card, dict):
        content_key = card.get("content_key")
    else:
        content_key = record.get("content_key")
    return "Facedown" if content_key is None else _name(content_key)


def _card_summary(cards: Iterable[FairCard]) -> str:
    counts = Counter(_card_name(card) for card in cards)
    if not counts:
        return "none"
    return ", ".join(f"{name} x{count}" if count > 1 else name for name, count in counts.items())


def _line(char: str = "-", width: int = 80) -> str:
    return char * width


def _section(lines: list[str], title: str) -> None:
    if lines and lines[-1] != "":
        lines.append("")
    lines.append(title)
    lines.append(_line())


def _context_line(observation: FairRunObservation) -> str:
    context = observation.context
    values = [
        f"Act {context.act}",
        f"Floor {context.floor}",
        f"Ascension {context.ascension}",
        f"HP {context.player_hp}/{context.player_max_hp}",
        f"Gold {context.gold}",
    ]
    return " | ".join(values)


def _run_inventory_lines(observation: FairRunObservation) -> list[str]:
    context = observation.context
    lines = [
        f"Deck ({len(context.deck)}): {_card_summary(context.deck)}",
        f"Relics ({len(context.relics)}): "
        + (", ".join(_name(relic.content_key) for relic in context.relics) or "none"),
    ]
    occupied = [slot for slot in context.potion_slots if slot.content_key is not None]
    potions = ", ".join(f"[{slot.slot}] {_name(slot.content_key)}" for slot in occupied)
    lines.append(f"Potions ({len(occupied)}/{len(context.potion_slots)}): {potions or 'none'}")
    return lines


def _run_screen_lines(observation: FairRunObservation) -> list[str]:
    screen = observation.screen
    lines: list[str] = []
    if observation.kind == "event":
        lines.append(f"Event: {_name(screen.get('event', 'unknown'))}")
        choices = _records(screen, "choices")
        lines.append("Choices:")
        lines.extend(
            f"  [{record.get('slot')}] {record.get('label', 'Unnamed option')}"
            for record in choices
        )
        match = _records(screen, "match_and_keep")
        if match:
            lines.append("Match and Keep:")
            for index, record in enumerate(match):
                status = (
                    "matched"
                    if record.get("matched")
                    else "revealed"
                    if record.get("revealed")
                    else "facedown"
                )
                lines.append(f"  [{index}] {_card_record_name(record)} ({status})")
        return lines

    if observation.kind == "map":
        lines.append(
            f"Map: current node {screen.get('current_node')}; "
            f"reachable {list(_integer_list(screen, 'reachable_nodes'))}"
        )
        lines.append("Nodes:")
        reachable = set(_integer_list(screen, "reachable_nodes"))
        for record in _records(screen, "nodes"):
            slot = record.get("slot")
            marker = "*" if slot in reachable else " "
            children = ", ".join(str(child) for child in _integers(record.get("children")))
            lines.append(
                f"  {marker}[{slot}] {_name(record.get('room_kind', 'unknown'))} -> [{children}]"
            )
        return lines

    if observation.kind == "reward":
        lines.append("Reward:")
        lines.append(
            f"  Gold: {screen.get('gold_offer', 0)} | "
            f"Stolen gold: {screen.get('stolen_gold_offer', 0)}"
        )
        for key, label in (
            ("potion_offer", "Potion"),
            ("relic_offer", "Relic"),
        ):
            value = screen.get(key)
            if value is not None:
                lines.append(f"  {label}: {_name(value)}")
        cards = _records(screen, "cards")
        if cards:
            lines.append("  Cards:")
            lines.extend(
                f"    [{record.get('slot')}] {_card_record_name(record)}" for record in cards
            )
        boss_relics = _strings(screen.get("boss_relic_choices"))
        if boss_relics:
            lines.append("  Boss relics: " + ", ".join(_name(relic) for relic in boss_relics))
        queued = _records(screen, "queued_card_rewards")
        if queued:
            lines.append(
                "  Queued card rewards: "
                + ", ".join(
                    f"[{record.get('slot')}] {record.get('choice_count', 0)} choices"
                    for record in queued
                )
            )
        return lines

    if observation.kind == "treasure":
        lines.append(
            f"Treasure: {_name(screen.get('chest_size', 'unknown'))} chest | "
            f"opened={screen.get('opened', False)}"
        )
        return lines

    if observation.kind == "rest":
        lines.append(f"Rest site: complete={screen.get('complete', False)}")
        lines.append(
            "Options: "
            + (", ".join(_name(option) for option in _strings(screen.get("options"))) or "none")
        )
        return lines

    if observation.kind == "shop":
        lines.append(f"Shop: merchant {'open' if screen.get('merchant_open') else 'closed'}")
        if screen.get("remove_cost") is not None:
            lines.append(f"  Remove cost: {screen['remove_cost']}")
        for key, label in (("cards", "Cards"), ("relics", "Relics"), ("potions", "Potions")):
            records = _records(screen, key)
            if records:
                lines.append(f"  {label}:")
                for record in records:
                    status = "sold" if record.get("sold") else f"{record.get('price')} gold"
                    lines.append(
                        f"    [{record.get('slot')}] {_name(record.get('content_key', 'unknown'))}"
                        f" ({status})"
                    )
        return lines

    if observation.kind == "grid":
        lines.append(f"Card selection: {_name(screen.get('purpose', 'selection'))}")
        selected = set(_integer_list(screen, "selected_indices"))
        for record in _records(screen, "cards"):
            slot = record.get("slot")
            marker = "selected" if slot in selected else "available"
            lines.append(f"  [{slot}] {_card_record_name(record)} ({marker})")
        return lines

    if observation.kind == "idle":
        lines.append("Idle: no active decision screen")
    elif observation.kind == "complete":
        lines.append("Run complete")
    return lines


def _combat_intent(monster: FairMonster) -> str:
    intent = monster.intent
    if intent.visibility == "hidden":
        return "hidden"
    if intent.visibility == "none":
        return "none"
    category = _name(intent.category or "unknown")
    if intent.damage is None:
        return category
    hits = intent.hits or 1
    return f"{category} {intent.damage}" + (f" x{hits}" if hits > 1 else "")


def _power_text(powers: tuple[FairPower, ...]) -> str:
    return ", ".join(f"{_name(power.key)} {power.amount}" for power in powers) or "none"


def _counter_text(counters: tuple[FairCounter, ...]) -> str:
    return ", ".join(f"{_name(counter.key)} {counter.value}" for counter in counters) or "none"


def _combat_lines(observation: FairCombatObservation) -> list[str]:
    lines = [
        (
            f"Player: HP {observation.player.hp}/{observation.player.max_hp} | "
            f"Block {observation.player.block} | "
            f"Energy {observation.player.energy}/{observation.player.max_energy}"
        ),
        f"Powers: {_power_text(observation.player.powers)}",
    ]
    if observation.orb_slots:
        orbs = []
        for slot in observation.orb_slots:
            if slot.orb is None:
                text = "empty"
            elif slot.orb.type == "dark":
                text = f"Dark (evoke {slot.orb.evoke})"
            else:
                text = _name(slot.orb.type)
            orbs.append(f"[{slot.slot}] {text}")
        lines.append("Orbs: " + ", ".join(orbs))
    _section(lines, f"Hand ({len(observation.hand)})")
    if observation.hand:
        lines.extend(f"  [{card.slot}] {_card_text(card.card)}" for card in observation.hand)
    else:
        lines.append("  none")

    _section(lines, "Monsters")
    if observation.monsters:
        for monster in observation.monsters:
            flags = [
                flag
                for flag, present in (
                    ("targetable", monster.targetable),
                    ("minion", monster.minion),
                    ("defensive", monster.in_defensive_mode),
                    ("escaped", monster.escaped),
                    ("dead", not monster.alive),
                )
                if present
            ]
            extras = []
            if monster.stolen_gold:
                extras.append(f"stolen gold {monster.stolen_gold}")
            if monster.stasis_card is not None:
                extras.append(f"stasis {_card_text(monster.stasis_card, include_cost=False)}")
            details = [
                f"HP {monster.hp}/{monster.max_hp}",
                f"Block {monster.block}",
                f"Intent {_combat_intent(monster)}",
                f"Powers {_power_text(monster.powers)}",
            ]
            if monster.slime_size is not None:
                details.append(_name(monster.slime_size))
            if flags:
                details.append("flags " + ", ".join(flags))
            if extras:
                details.extend(extras)
            lines.append(
                f"  [{monster.slot}] {_name(monster.content_key)} | " + " | ".join(details)
            )
    else:
        lines.append("  none")

    _section(lines, "Piles")
    for name, pile in (
        ("Draw", observation.draw_pile),
        ("Discard", observation.discard_pile),
        ("Exhaust", observation.exhaust_pile),
    ):
        summary = _card_summary(pile.cards)
        lines.append(f"  {name} ({pile.count}): {summary}")
    if observation.draw_pile.known_order:
        lines.append(
            "  Known draw order: "
            + " -> ".join(_card_name(card) for card in observation.draw_pile.known_order)
        )

    _section(lines, "Relics")
    lines.extend(
        f"  [{relic.slot}] {_name(relic.content_key)}"
        + (f" | {_counter_text(relic.state)}" if relic.state else "")
        for relic in observation.relics
    )
    if not observation.relics:
        lines.append("  none")

    _section(lines, "Potions")
    lines.extend(
        f"  [{potion.slot}] "
        + (_name(potion.content_key) if potion.content_key is not None else "empty")
        for potion in observation.potion_slots
    )
    return lines


def action_label(decision: Decision, action: Action) -> str:
    """Return a readable label derived only from the matching fair decision."""

    observation = decision.observation
    if isinstance(observation, FairRunObservation):
        screen = observation.screen
        if action.kind == "event_choose":
            option = next(
                (
                    record
                    for record in _records(screen, "choices")
                    if record.get("slot") == action.option_slot
                ),
                None,
            )
            return (
                f"Choose {_humanize((option or {}).get('label', f'option {action.option_slot}'))}"
            )
        if action.kind == "choose_map_node":
            node = next(
                (
                    record
                    for record in _records(screen, "nodes")
                    if record.get("slot") == action.node_slot
                ),
                None,
            )
            return f"Choose {_name((node or {}).get('room_kind', 'map node'))}"
        if action.kind == "select_grid_card":
            card = next(
                (
                    record
                    for record in _records(screen, "cards")
                    if record.get("slot") == action.option_slot
                ),
                None,
            )
            return f"Select {_card_record_name(card or {})}"
        labels = {
            "take_card_reward": "Take card reward",
            "take_potion_reward": "Take potion reward",
            "take_relic_reward": "Take relic reward",
            "take_relic_reward_at": "Take relic reward",
            "choose_boss_relic_reward": "Choose boss relic",
            "open_queued_card_reward": "Open queued card reward",
            "open_chest": "Open chest",
            "proceed": "Proceed",
            "rest_heal": "Rest",
            "rest_open_smith": "Smith a card",
            "rest_open_remove": "Remove a card",
            "rest_smith": "Smith selected card",
            "rest_remove_card": "Remove selected card",
            "rest_lift": "Lift",
            "rest_dig": "Dig",
            "rest_proceed": "Leave rest site",
            "buy_shop_card": "Buy card",
            "buy_shop_relic": "Buy relic",
            "buy_shop_potion": "Buy potion",
            "enter_shop": "Enter shop",
            "leave_shop": "Leave shop",
            "open_shop_remove": "Remove a card",
        }
        return labels.get(action.kind, _name(action.kind))

    if not isinstance(observation, FairCombatObservation):
        return _name(action.kind)
    match action.kind:
        case "play_hand_slot":
            card = next(
                (visible.card for visible in observation.hand if visible.slot == action.hand_slot),
                None,
            )
            label = (
                f"Play {_card_name(card) if card is not None else f'hand slot {action.hand_slot}'}"
            )
        case "end_turn":
            return "End turn"
        case "use_potion_slot":
            potion = next(
                (
                    slot.content_key
                    for slot in observation.potion_slots
                    if slot.slot == action.potion_slot
                ),
                None,
            )
            label = f"Use {_name(potion) if potion is not None else f'potion slot {action.potion_slot}'}"
        case "discard_potion_slot":
            potion = next(
                (
                    slot.content_key
                    for slot in observation.potion_slots
                    if slot.slot == action.potion_slot
                ),
                None,
            )
            return f"Discard {_name(potion) if potion is not None else f'potion slot {action.potion_slot}'}"
        case "toggle_visible_card" | "choose_visible_option":
            option = next(
                (
                    selection.card
                    for selection in (
                        observation.selection.options if observation.selection else ()
                    )
                    if selection.slot == action.option_slot
                ),
                None,
            )
            verb = "Toggle" if action.kind == "toggle_visible_card" else "Choose"
            return f"{verb} {_card_name(option) if option is not None else f'option {action.option_slot}'}"
        case "confirm_selection":
            return "Confirm selection"
        case "skip_selection":
            return "Skip selection"
        case _:
            labels = {
                "event_choose": "Choose event option",
                "select_grid_card": "Select card",
                "confirm_grid": "Confirm card selection",
                "cancel_grid": "Cancel card selection",
                "choose_map_node": "Choose map node",
                "rest_heal": "Rest",
                "skip_reward": "Skip reward",
                "close_card_reward": "Close card reward",
                "take_card_reward": "Take card reward",
                "take_singing_bowl_reward": "Take Singing Bowl reward",
                "take_gold_reward": "Take gold reward",
                "take_stolen_gold_reward": "Take stolen gold",
            }
            label = labels.get(action.kind, _name(action.kind))

    if action.target_slot is not None:
        target = next(
            (monster for monster in observation.monsters if monster.slot == action.target_slot),
            None,
        )
        target_name = (
            _name(target.content_key)
            if target is not None
            else f"monster slot {action.target_slot}"
        )
        return f"{label} -> {target_name}"
    return label


def _action_descriptor(action: Action) -> str:
    fields = [
        ("hand", action.hand_slot),
        ("potion", action.potion_slot),
        ("option", action.option_slot),
        ("target", action.target_slot),
        ("card", action.card_slot),
        ("node", action.node_slot),
        ("reward", action.reward_slot),
        ("shop", action.shop_slot),
    ]
    return ", ".join(f"{name}={value}" for name, value in fields if value is not None)


def format_action(decision: Decision, action: Action, *, index: int | None = None) -> str:
    """Format one legal action using the fair decision that produced it."""

    prefix = f"[{index}] " if index is not None else ""
    descriptor = _action_descriptor(action)
    suffix = f" | {descriptor}" if descriptor else ""
    return f"{prefix}{action_label(decision, action)} ({action.kind}{suffix})"


def format_actions(decision: Decision) -> str:
    """Format all legal actions from a decision as an indexed text list."""

    if not decision.actions:
        return "none"
    return "\n".join(
        f"  {format_action(decision, action, index=index)}"
        for index, action in enumerate(decision.actions)
    )


def show_actions(decision: Decision) -> None:
    """Print the legal actions from a decision."""

    print(format_actions(decision))


def _observation_lines(
    observation: FairCombatObservation | FairRunObservation,
    *,
    revision: int | None = None,
) -> list[str]:
    if isinstance(observation, FairRunObservation):
        title = f"{_name(observation.kind)} | phase={observation.phase}"
        if revision is not None:
            title += f" | revision={revision}"
        lines = [title, _context_line(observation), _line()]
        lines.extend(_run_screen_lines(observation))
        _section(lines, "Inventory")
        lines.extend(_run_inventory_lines(observation))
        return lines

    title = f"Combat | phase={_humanize(observation.phase)}"
    if revision is not None:
        title += f" | revision={revision}"
    context = observation.context
    lines = [
        title,
        f"Act {context.act} | Floor {context.floor} | Ascension {context.ascension} | Gold {context.gold}",
        _line(),
    ]
    lines.extend(_combat_lines(observation))
    return lines


def format_observation(observation: FairCombatObservation | FairRunObservation) -> str:
    """Format one fair observation as readable plain text."""

    return "\n".join(_observation_lines(observation))


def format_decision(decision: Decision) -> str:
    """Format a fair observation and its legal actions as plain text."""

    lines = _observation_lines(decision.observation, revision=decision.revision)
    _section(lines, f"Legal actions ({len(decision.actions)})")
    lines.extend(format_actions(decision).splitlines())
    return "\n".join(lines)


def show_decision(decision: Decision) -> None:
    """Print a complete fair decision in Jupyter, a terminal, or a log."""

    print(format_decision(decision))
