"""Read-only helpers for CommunicationMod JSONL trace replay."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import json
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[3]
TRACE_ROOT = REPO_ROOT / "verification" / "corpus" / "communication_mod"


@dataclass(frozen=True)
class TraceReplayStore:
    root: Path = TRACE_ROOT

    @classmethod
    def default(cls) -> "TraceReplayStore":
        return cls()

    def list_traces(self, limit: int = 50) -> dict[str, Any]:
        traces = [
            self._trace_metadata(path)
            for path in sorted(
                self._root().glob("*.jsonl"),
                key=lambda candidate: candidate.stat().st_mtime,
                reverse=True,
            )[: max(0, min(limit, 200))]
        ]
        return {"root": str(self._root()), "traces": traces}

    def load_trace(self, trace_id: str, offset: int = 0, limit: int = 200) -> dict[str, Any]:
        path = self._resolve_trace(trace_id)
        metadata = self._trace_metadata(path)
        start = max(0, offset)
        stop = start + max(0, min(limit, 1000))
        records = []
        for index, record in enumerate(self._iter_records(path)):
            if index < start:
                continue
            if index >= stop:
                break
            records.append(record)
        return {
            "trace": metadata,
            "offset": start,
            "limit": stop - start,
            "records": records,
        }

    def _trace_metadata(self, path: Path) -> dict[str, Any]:
        counts = {
            "records": 0,
            "states": 0,
            "actions": 0,
            "parse_errors": 0,
            "first_step": None,
            "last_step": None,
            "summary": None,
        }
        for record in self._iter_records(path, include_raw=False):
            counts["records"] += 1
            record_type = record["type"]
            if record_type == "state":
                counts["states"] += 1
                counts["summary"] = record.get("summary")
            elif record_type == "action":
                counts["actions"] += 1
            elif record_type == "parse_error":
                counts["parse_errors"] += 1

            step = record.get("step")
            if step is not None:
                counts["first_step"] = step if counts["first_step"] is None else min(counts["first_step"], step)
                counts["last_step"] = step if counts["last_step"] is None else max(counts["last_step"], step)

        stat = path.stat()
        return {
            "id": path.name,
            "name": path.name,
            "bytes": stat.st_size,
            "modified_at": datetime.fromtimestamp(stat.st_mtime, timezone.utc).isoformat(),
            **counts,
        }

    def _iter_records(self, path: Path, include_raw: bool = True) -> list[dict[str, Any]]:
        records = []
        with path.open("r", encoding="utf-8") as handle:
            for line_number, line in enumerate(handle, start=1):
                line = line.strip()
                if not line:
                    continue
                try:
                    raw = json.loads(line)
                    records.append(_summarize_record(line_number, raw, include_raw=include_raw))
                except json.JSONDecodeError as error:
                    records.append(
                        {
                            "line": line_number,
                            "type": "parse_error",
                            "step": None,
                            "timestamp": None,
                            "summary": {"error": str(error)},
                            "raw": line if include_raw else None,
                        }
                    )
        return records

    def _resolve_trace(self, trace_id: str) -> Path:
        if not trace_id or Path(trace_id).name != trace_id or not trace_id.endswith(".jsonl"):
            raise KeyError(f"unknown trace: {trace_id}")
        path = (self._root() / trace_id).resolve()
        root = self._root()
        if root not in path.parents or not path.is_file():
            raise KeyError(f"unknown trace: {trace_id}")
        return path

    def _root(self) -> Path:
        return self.root.resolve()


def _summarize_record(line_number: int, raw: dict[str, Any], include_raw: bool) -> dict[str, Any]:
    record_type = str(raw.get("type", "unknown"))
    timestamp = raw.get("received_at") or raw.get("sent_at") or raw.get("started_at") or raw.get("ended_at")
    record = {
        "line": line_number,
        "type": record_type,
        "step": raw.get("step"),
        "timestamp": timestamp,
        "command": raw.get("command"),
        "summary": _record_summary(raw),
    }
    if include_raw:
        record["raw"] = raw
    return record


def _record_summary(raw: dict[str, Any]) -> dict[str, Any]:
    record_type = raw.get("type")
    if record_type == "state":
        message = raw.get("message") or {}
        game_state = message.get("game_state") or {}
        combat = game_state.get("combat_state") or {}
        player = combat.get("player") or {}
        monsters = combat.get("monsters") or []
        return {
            "ready_for_command": message.get("ready_for_command"),
            "in_game": message.get("in_game"),
            "available_commands": message.get("available_commands") or [],
            "screen_type": game_state.get("screen_type"),
            "room_phase": game_state.get("room_phase"),
            "action_phase": game_state.get("action_phase"),
            "floor": game_state.get("floor"),
            "act": game_state.get("act"),
            "hp": _hp(game_state),
            "gold": game_state.get("gold"),
            "choices": game_state.get("choice_list") or [],
            "combat": {
                "energy": combat.get("energy"),
                "hand": _names(combat.get("hand") or []),
                "monsters": [_monster_summary(monster) for monster in monsters],
                "player_hp": player.get("current_hp") or combat.get("player_hp"),
                "player_block": player.get("block") or combat.get("player_block"),
            }
            if combat
            else None,
        }
    if record_type == "action":
        return {"command": raw.get("command")}
    if record_type == "metadata":
        return {
            "schema": raw.get("schema"),
            "source": raw.get("source"),
            "client": raw.get("client"),
            "event": raw.get("event"),
            "reason": raw.get("reason"),
        }
    return {}


def _hp(game_state: dict[str, Any]) -> str | None:
    current = game_state.get("current_hp")
    maximum = game_state.get("max_hp")
    if current is None and maximum is None:
        return None
    return f"{current}/{maximum}"


def _names(cards: list[dict[str, Any]]) -> list[str]:
    return [str(card.get("name") or card.get("id") or "?") for card in cards]


def _monster_summary(monster: dict[str, Any]) -> dict[str, Any]:
    return {
        "name": monster.get("name") or monster.get("id"),
        "hp": _hp(monster),
        "block": monster.get("block"),
        "intent": monster.get("intent"),
    }
