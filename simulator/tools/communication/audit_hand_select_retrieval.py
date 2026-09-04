#!/usr/bin/env python3
"""Audit immutable traces for multiplied-delta hand-selection corruption."""

from __future__ import annotations

import argparse
import json
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

CardKey = tuple[str, int, int]
CardCounts = Counter[CardKey]
ContentCounts = Counter[str]


def trace_id(path: Path) -> str:
    return path.name.split("-", 1)[0]


def card_counts(cards: list[dict[str, Any]]) -> CardCounts:
    return Counter(
        (
            str(card.get("id") or card.get("name") or ""),
            int(card.get("upgrades") or 0),
            int(card.get("misc") or 0),
        )
        for card in cards
    )


def content_counts(cards: CardCounts) -> ContentCounts:
    result: ContentCounts = Counter()
    for (content_id, _upgrades, _misc), count in cards.items():
        result[content_id] += count
    return result


@dataclass(frozen=True)
class TraceState:
    screen: str | None
    current_action: str | None
    hand: CardCounts
    draw: CardCounts
    discard: CardCounts
    exhaust: CardCounts
    relics: frozenset[str]
    has_combat: bool

    def visible_cards(self) -> CardCounts:
        return self.hand + self.draw + self.discard + self.exhaust


def state_summary(record: dict[str, Any]) -> TraceState:
    message = record.get("message") or {}
    game_state = message.get("game_state") or {}
    combat = game_state.get("combat_state") or {}
    return TraceState(
        screen=game_state.get("screen_type"),
        current_action=message.get("current_action"),
        hand=card_counts(combat.get("hand") or []),
        draw=card_counts(combat.get("draw_pile") or []),
        discard=card_counts(combat.get("discard_pile") or []),
        exhaust=card_counts(combat.get("exhaust_pile") or []),
        relics=frozenset(str(relic.get("id") or "") for relic in game_state.get("relics") or []),
        has_combat="hand" in combat,
    )


def contains_counts(destination: Counter[Any], selected: Counter[Any]) -> bool:
    return all(destination[card] >= count for card, count in selected.items())


def audit_trace(path: Path) -> tuple[Counter[str], list[tuple[int, str]]]:
    states: dict[int, TraceState] = {}
    actions: dict[int, str] = {}
    with path.open(encoding="utf-8") as source:
        for line in source:
            record = json.loads(line)
            step = record.get("step")
            if not isinstance(step, int):
                continue
            if record.get("type") == "state":
                states[step] = state_summary(record)
            elif record.get("type") == "action":
                actions[step] = str(record.get("command") or "").strip().upper()

    confirms: Counter[str] = Counter()
    skipped: list[tuple[int, str]] = []
    for step, command in actions.items():
        if command != "CONFIRM":
            continue
        before = states.get(step - 1)
        after = states.get(step)
        if before is None or after is None or not before.has_combat or not after.has_combat:
            continue
        if before.screen != "HAND_SELECT" or before.current_action is None:
            continue

        cursor = step - 1
        while states.get(cursor - 1) is not None and states[cursor - 1].screen == "HAND_SELECT":
            cursor -= 1
        selected = states[cursor].hand - before.hand
        if not selected:
            continue

        owner = before.current_action
        confirms[owner] += 1
        visible_gain = content_counts(after.visible_cards()) - content_counts(
            before.visible_cards()
        )
        retrieval_missing = not contains_counts(visible_gain, content_counts(selected))

        # For ExhaustAction, pin the exact selected card state to its target
        # pile. This catches cases masked by drawing another copy with the same
        # content identity after skipped retrieval.
        if owner == "ExhaustAction":
            destination_gain = after.exhaust - before.exhaust
            if "Strange Spoon" in after.relics:
                destination_gain += after.discard - before.discard
            retrieval_missing |= not contains_counts(destination_gain, selected)

        if retrieval_missing:
            skipped.append((step, owner))
    return confirms, skipped


def trace_paths(root: Path) -> Iterable[Path]:
    if root.is_file():
        yield root
    else:
        yield from sorted(root.glob("*.jsonl"))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", type=Path, help="trace JSONL file or corpus directory")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args()

    confirms: Counter[str] = Counter()
    skipped_by_action: Counter[str] = Counter()
    affected: dict[str, list[dict[str, Any]]] = {}
    for path in trace_paths(args.path):
        trace_confirms, trace_skips = audit_trace(path)
        confirms.update(trace_confirms)
        if trace_skips:
            affected[trace_id(path)] = [
                {"step": step, "current_action": owner} for step, owner in trace_skips
            ]
            skipped_by_action.update(owner for _, owner in trace_skips)

    total_confirms = sum(confirms.values())
    total_skipped = sum(skipped_by_action.values())
    result = {
        "selected_hand_action_confirms": total_confirms,
        "skipped_retrieval_confirms": total_skipped,
        "skip_rate": total_skipped / total_confirms if total_confirms else 0.0,
        "confirms_by_action": dict(sorted(confirms.items())),
        "skips_by_action": dict(sorted(skipped_by_action.items())),
        "affected_trace_count": len(affected),
        "affected_traces": affected,
    }
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(
            f"selected hand-action confirms={total_confirms} skipped={total_skipped} "
            f"rate={100 * result['skip_rate']:.1f}% traces={len(affected)}"
        )
        print(f"confirms_by_action={dict(sorted(confirms.items()))}")
        print(f"skips_by_action={dict(sorted(skipped_by_action.items()))}")
        for affected_id in affected:
            print(affected_id)
    return 1 if affected else 0


if __name__ == "__main__":
    raise SystemExit(main())
