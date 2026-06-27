"""Small parity checks between simulator state and observed bridge state."""

from __future__ import annotations

from typing import Any


def combat_parity(sim_state: dict[str, Any], bridge_status: dict[str, Any]) -> dict[str, Any]:
    summary = bridge_status.get("summary")
    if not isinstance(summary, dict) or summary.get("missing"):
        return _unknown("missing bridge summary")
    combat = summary.get("combat")
    if not isinstance(combat, dict):
        return _unknown("bridge summary has no combat state")

    diffs: list[dict[str, Any]] = []
    player = sim_state.get("player", {})
    _compare(diffs, "player.hp", player.get("hp"), combat.get("player_hp"))
    _compare(diffs, "player.block", player.get("block"), combat.get("player_block"))
    _compare(diffs, "player.energy", player.get("energy"), combat.get("energy"))

    sim_monsters = [monster for monster in sim_state.get("monsters", []) if monster.get("alive", True)]
    observed_monsters = [monster for monster in combat.get("monsters", []) if not monster.get("gone", False)]
    _compare(diffs, "monsters.count", len(sim_monsters), len(observed_monsters))
    for index, (sim_monster, observed_monster) in enumerate(zip(sim_monsters, observed_monsters)):
        _compare(diffs, f"monsters.{index}.hp", sim_monster.get("hp"), observed_monster.get("hp"))
        _compare(diffs, f"monsters.{index}.block", sim_monster.get("block"), observed_monster.get("block"))

    return {
        "status": "diverged" if diffs else "in_sync",
        "diffs": diffs,
        "observed_step": summary.get("step"),
        "bridge_stale": bridge_status.get("stale"),
    }


def _unknown(reason: str) -> dict[str, Any]:
    return {"status": "unknown", "reason": reason, "diffs": []}


def _compare(diffs: list[dict[str, Any]], path: str, simulator: Any, observed: Any) -> None:
    if observed is None:
        return
    if simulator != observed:
        diffs.append({"path": path, "simulator": simulator, "observed": observed})
