"""Deterministic simulator self-play trace generation and replay."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import argparse
import json
import random
from pathlib import Path
from typing import Any, Iterable

from sts import omni
from sts.search import CombatSearchConfig, search_combat


DEFAULT_COMBAT_POLICY = CombatSearchConfig(
    max_depth=40,
    objective="aggressive_lethal",
    algorithm="portfolio",
    beam_width=12,
)


@dataclass(frozen=True)
class SelfPlayResult:
    trace_path: Path | None
    steps: int
    stop_reason: str
    final_phase: str | None
    verified: bool


def run_self_play(
    *,
    output: Path,
    seed: str | None = None,
    ascension: int = 0,
    start: str = "seed",
    random_seed: int = 0,
    max_steps: int = 200,
    combat_policy: CombatSearchConfig = DEFAULT_COMBAT_POLICY,
) -> SelfPlayResult:
    """Run one simulator self-play episode and write a replayable JSONL trace."""

    rng = random.Random(random_seed)
    output.parent.mkdir(parents=True, exist_ok=True)
    started_at = _now()

    try:
        env = _start_env(start=start, seed=seed, ascension=ascension)
    except Exception as error:
        _write_jsonl(
            output,
            [
                _metadata(
                    started_at=started_at,
                    seed=seed,
                    ascension=ascension,
                    start=start,
                    random_seed=random_seed,
                    max_steps=max_steps,
                    combat_policy=combat_policy,
                    stop_reason=f"unsupported_start: {error}",
                )
            ],
        )
        return SelfPlayResult(output, 0, f"unsupported_start: {error}", None, False)

    records: list[dict[str, Any]] = [
        _metadata(
            started_at=started_at,
            seed=seed,
            ascension=ascension,
            start=start,
            random_seed=random_seed,
            max_steps=max_steps,
            combat_policy=combat_policy,
            initial_snapshot_json=env.snapshot_json(),
            initial_state_id=env.snapshot_hash(),
        )
    ]
    stop_reason = "max_steps"
    steps = 0

    for step in range(max_steps):
        before_hash = env.snapshot_hash()
        actions = env.exact_legal_actions()
        if not actions:
            unsupported = env.unsupported_reason()
            stop_reason = f"unsupported_transition: {unsupported}" if unsupported else "no_legal_actions"
            break

        try:
            action, policy_name, policy_diagnostics = _choose_action(
                env,
                actions,
                rng,
                combat_policy,
            )
        except Exception as error:
            stop_reason = f"policy_error: {error}"
            break

        before_snapshot = env.snapshot_json()
        before_summary = _summary(env)
        try:
            result = env.step(action)
        except Exception as error:
            records.append(
                _step_record(
                    step=step,
                    before_hash=before_hash,
                    before_snapshot_json=before_snapshot,
                    before_summary=before_summary,
                    legal_actions=actions,
                    action=action,
                    policy_name=policy_name,
                    policy_diagnostics=policy_diagnostics,
                    error=f"step_error: {error}",
                )
            )
            stop_reason = f"step_error: {error}"
            break

        records.append(
            _step_record(
                step=step,
                before_hash=before_hash,
                before_snapshot_json=before_snapshot,
                before_summary=before_summary,
                legal_actions=actions,
                action=action,
                policy_name=policy_name,
                policy_diagnostics=policy_diagnostics,
                after_hash=result.snapshot_hash,
                after_snapshot_json=result.snapshot_json,
                after_summary=_summary(env),
                transition=result.transition,
                unsupported_reason=result.unsupported_reason,
            )
        )
        steps += 1

        if result.unsupported_reason and not env.exact_legal_actions():
            stop_reason = f"unsupported_transition: {result.unsupported_reason}"
            break
        phase = env.phase()
        if phase in {"won", "lost"}:
            stop_reason = phase
            break
    else:
        stop_reason = "max_steps"

    records[0]["ended_at"] = _now()
    records[0]["stop_reason"] = stop_reason
    records[0]["steps"] = steps
    records[0]["final_phase"] = env.phase()
    records[0]["final_state_id"] = env.snapshot_hash()
    records[0]["final_summary"] = _summary(env)
    _write_jsonl(output, records)
    verification = verify_self_play_trace(output)
    return SelfPlayResult(output, steps, stop_reason, env.phase(), verification["ok"])


def verify_self_play_trace(path: Path) -> dict[str, Any]:
    """Replay a generated self-play trace from its initial snapshot."""

    records = _read_jsonl(path)
    if not records:
        return {"ok": False, "error": "empty trace", "steps": 0}
    metadata = records[0]
    if metadata.get("type") != "metadata":
        return {"ok": False, "error": "first record is not metadata", "steps": 0}
    initial_snapshot = metadata.get("initial_snapshot_json")
    if not isinstance(initial_snapshot, str):
        return {"ok": False, "error": "metadata has no initial_snapshot_json", "steps": 0}

    env = omni.OmniRunEnv.from_snapshot_json(initial_snapshot)
    steps = 0
    for record in records[1:]:
        if record.get("type") != "step":
            continue
        before_hash = record.get("before_hash")
        if env.snapshot_hash() != before_hash:
            return {
                "ok": False,
                "error": "before hash mismatch",
                "step": record.get("step"),
                "expected": before_hash,
                "actual": env.snapshot_hash(),
            }
        action_json = record.get("action_json")
        action = _find_action(env.exact_legal_actions(), action_json)
        if action is None:
            return {
                "ok": False,
                "error": "action not legal during replay",
                "step": record.get("step"),
                "action_json": action_json,
            }
        result = env.step(action)
        after_hash = record.get("after_hash")
        if after_hash and result.snapshot_hash != after_hash:
            return {
                "ok": False,
                "error": "after hash mismatch",
                "step": record.get("step"),
                "expected": after_hash,
                "actual": result.snapshot_hash,
            }
        steps += 1

    return {
        "ok": True,
        "steps": steps,
        "final_state_id": env.snapshot_hash(),
        "final_phase": env.phase(),
        "metadata_stop_reason": metadata.get("stop_reason"),
    }


def _start_env(*, start: str, seed: str | None, ascension: int) -> Any:
    if start == "seed":
        return omni.OmniRunEnv.new_ironclad(seed=seed, ascension=ascension)
    if start == "map_fixture":
        return omni.OmniRunEnv.map_fixture()
    if start == "combat_fixture":
        return omni.OmniRunEnv.combat_fixture()
    raise ValueError(f"unsupported start mode: {start}")


def _choose_action(
    env: Any,
    actions: list[Any],
    rng: random.Random,
    combat_policy: CombatSearchConfig,
) -> tuple[Any, str, dict[str, Any]]:
    if env.phase() == "combat":
        recommendation = search_combat(env, combat_policy)
        if recommendation.best_action is not None:
            return (
                recommendation.best_action,
                "portfolio_combat",
                {
                    "score": recommendation.score,
                    "nodes": recommendation.nodes,
                    "terminal_reason": recommendation.terminal_reason,
                    **recommendation.diagnostics,
                },
            )

    action = _random_viable_noncombat_action(env, actions, rng)
    return action, "random_noncombat", {"candidate_count": len(actions)}


def _random_viable_noncombat_action(env: Any, actions: list[Any], rng: random.Random) -> Any:
    shuffled = list(actions)
    rng.shuffle(shuffled)
    fallback = shuffled[0]
    for action in shuffled:
        child = env.clone()
        try:
            result = child.step(action)
        except Exception:
            continue
        if result.unsupported_reason and not child.exact_legal_actions():
            continue
        return action
    return fallback


def _find_action(actions: Iterable[Any], action_json: Any) -> Any | None:
    for action in actions:
        if action.json() == action_json:
            return action
    return None


def _metadata(
    *,
    started_at: str,
    seed: str | None,
    ascension: int,
    start: str,
    random_seed: int,
    max_steps: int,
    combat_policy: CombatSearchConfig,
    initial_snapshot_json: str | None = None,
    initial_state_id: str | None = None,
    stop_reason: str | None = None,
) -> dict[str, Any]:
    return {
        "type": "metadata",
        "schema": 1,
        "source": "sim_selfplay",
        "started_at": started_at,
        "seed": seed,
        "ascension": ascension,
        "start": start,
        "random_seed": random_seed,
        "max_steps": max_steps,
        "combat_policy": combat_policy.__dict__,
        "noncombat_policy": "random_viable_v1",
        "initial_state_id": initial_state_id,
        "initial_snapshot_json": initial_snapshot_json,
        "stop_reason": stop_reason,
    }


def _step_record(
    *,
    step: int,
    before_hash: str,
    before_snapshot_json: str,
    before_summary: dict[str, Any],
    legal_actions: list[Any],
    action: Any,
    policy_name: str,
    policy_diagnostics: dict[str, Any],
    after_hash: str | None = None,
    after_snapshot_json: str | None = None,
    after_summary: dict[str, Any] | None = None,
    transition: Any | None = None,
    unsupported_reason: str | None = None,
    error: str | None = None,
) -> dict[str, Any]:
    return {
        "type": "step",
        "step": step,
        "before_hash": before_hash,
        "before_snapshot_json": before_snapshot_json,
        "before_summary": before_summary,
        "legal_actions": [_action_record(action) for action in legal_actions],
        "action_family": action.family(),
        "action_kind": action.kind(),
        "action_json": action.json(),
        "policy": policy_name,
        "policy_diagnostics": policy_diagnostics,
        "after_hash": after_hash,
        "after_snapshot_json": after_snapshot_json,
        "after_summary": after_summary,
        "transition": _transition_record(transition),
        "unsupported_reason": unsupported_reason,
        "error": error,
    }


def _action_record(action: Any) -> dict[str, Any]:
    return {"family": action.family(), "kind": action.kind(), "json": action.json()}


def _transition_record(transition: Any | None) -> dict[str, Any] | None:
    if transition is None:
        return None
    return {
        "action_json": transition.action_json,
        "previous_hash": transition.previous_hash,
        "resulting_hash": transition.resulting_hash,
        "events_json": transition.events_json,
        "rng_draws_json": transition.rng_draws_json,
        "simulator_error": transition.simulator_error,
    }


def _summary(env: Any) -> dict[str, Any]:
    state = json.loads(env.state_json())
    run = state.get("state", state)
    combat = run.get("combat") or {}
    player = combat.get("player") or {}
    return {
        "phase": env.phase(),
        "decision": env.current_decision(),
        "unsupported_reason": env.unsupported_reason(),
        "player_hp": run.get("player_hp") if run.get("player_hp") is not None else player.get("hp"),
        "player_max_hp": run.get("player_max_hp")
        if run.get("player_max_hp") is not None
        else player.get("max_hp"),
        "gold": run.get("gold"),
        "floor": run.get("current_floor"),
        "act": run.get("current_act"),
        "potions": run.get("potions") or [],
        "relics": run.get("relics") or [],
        "combat": {
            "energy": player.get("energy"),
            "hand": [card.get("content_id") for card in ((combat.get("piles") or {}).get("hand") or [])],
            "monsters": [
                {
                    "id": monster.get("id"),
                    "hp": monster.get("hp"),
                    "block": monster.get("block"),
                    "alive": monster.get("alive"),
                    "intent": monster.get("intent"),
                }
                for monster in combat.get("monsters", [])
            ],
        }
        if combat
        else None,
    }


def _write_jsonl(path: Path, records: Iterable[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, sort_keys=True))
            handle.write("\n")


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    with path.open("r", encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Generate or verify simulator self-play traces.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser("run")
    run_parser.add_argument("--output", type=Path, required=True)
    run_parser.add_argument("--seed")
    run_parser.add_argument("--ascension", type=int, default=0)
    run_parser.add_argument("--start", choices=["seed", "map_fixture", "combat_fixture"], default="seed")
    run_parser.add_argument("--random-seed", type=int, default=0)
    run_parser.add_argument("--max-steps", type=int, default=200)

    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("trace", type=Path)

    args = parser.parse_args(argv)
    if args.command == "run":
        result = run_self_play(
            output=args.output,
            seed=args.seed,
            ascension=args.ascension,
            start=args.start,
            random_seed=args.random_seed,
            max_steps=args.max_steps,
        )
        print(json.dumps(result.__dict__ | {"trace_path": str(result.trace_path)}, indent=2))
    elif args.command == "verify":
        print(json.dumps(verify_self_play_trace(args.trace), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
