import json
from typing import cast

import pytest

from sts_sim import (
    FairCombatContext,
    FairCombatObservation,
    FairRunContext,
    FairRunObservation,
    InvalidChoiceError,
    RunEnv,
    StaleDecisionError,
)


def test_fair_decision_is_typed_and_atomic() -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()

    assert isinstance(decision.observation, FairCombatObservation)
    assert isinstance(decision.observation.hand, tuple)
    assert decision.observation.schema_version == 2
    assert isinstance(decision.observation.orb_slots, tuple)
    assert isinstance(decision.observation.context, FairCombatContext)
    assert decision.revision == 0
    assert decision.observation.hand
    assert all(action.kind for action in decision.actions)


def test_fair_step_advances_revision_without_exposing_state_json() -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()
    end_turn = next(action for action in decision.actions if action.kind == "end_turn")

    result = env.step(end_turn)

    assert result.combat_outcome is None
    assert env.revision == 1
    assert not hasattr(env, "state_json")


def test_fair_step_rejects_stale_revision() -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()
    end_turn = next(action for action in decision.actions if action.kind == "end_turn")

    env.step(end_turn)
    with pytest.raises(StaleDecisionError, match="public run decision is stale"):
        env.step(end_turn)


def test_fair_step_rejects_an_action_from_a_different_screen() -> None:
    env = RunEnv.combat_fixture()
    foreign = RunEnv.map_fixture().decision().actions[0]

    with pytest.raises(InvalidChoiceError, match="public run action is invalid"):
        env.step(foreign)


def _combat_screen() -> dict[str, object]:
    payload = json.loads(RunEnv.combat_fixture()._native.observation_json())
    screen = payload["screen"]
    assert isinstance(screen, dict)
    return cast(dict[str, object], screen)


def _run_observation_payload() -> dict[str, object]:
    payload = json.loads(RunEnv.map_fixture()._native.observation_json())
    assert isinstance(payload, dict)
    return cast(dict[str, object], payload)


def _mapping(value: object) -> dict[str, object]:
    assert isinstance(value, dict)
    return cast(dict[str, object], value)


def _sequence(value: object) -> list[object]:
    assert isinstance(value, list)
    return list(value)


def test_fair_combat_observation_schema_is_strict() -> None:
    screen = _combat_screen()
    parsed = FairCombatObservation._from_payload(screen)
    assert parsed.schema_version == 2
    extra = dict(screen)
    extra["unknown"] = 1
    with pytest.raises(ValueError, match="unknown field"):
        FairCombatObservation._from_payload(extra)
    missing = dict(screen)
    missing.pop("phase")
    with pytest.raises(ValueError, match="missing"):
        FairCombatObservation._from_payload(missing)
    wrong_schema = dict(screen)
    wrong_schema["schema_version"] = 1
    with pytest.raises(ValueError, match="unsupported fair combat observation schema"):
        FairCombatObservation._from_payload(wrong_schema)
    bool_gold = dict(screen)
    bool_gold["context"] = dict(_mapping(screen["context"]))
    _mapping(bool_gold["context"])["gold"] = True
    with pytest.raises(TypeError, match="integer"):
        FairCombatObservation._from_payload(bool_gold)
    extra_context = dict(screen)
    extra_context["context"] = dict(_mapping(screen["context"]))
    _mapping(extra_context["context"])["player_hp"] = 80
    with pytest.raises(ValueError, match="unknown field"):
        FairCombatObservation._from_payload(extra_context)
    invalid_phase = dict(screen)
    invalid_phase["phase"] = "nope"
    with pytest.raises(ValueError, match="invalid"):
        FairCombatObservation._from_payload(invalid_phase)
    hand = _sequence(screen["hand"])
    first_hand = _mapping(hand[0])
    card = dict(_mapping(first_hand["card"]))
    dynamic = dict(_mapping(card["dynamic"]))
    dynamic["rampage_damage_bonus"] = None
    card["dynamic"] = dynamic
    hand_card = dict(first_hand)
    hand_card["card"] = card
    null_dynamic = dict(screen)
    null_dynamic["hand"] = [hand_card, *hand[1:]]
    with pytest.raises(TypeError, match="omitted rather than null"):
        FairCombatObservation._from_payload(null_dynamic)
    extra_player = dict(screen)
    extra_player["player"] = dict(_mapping(screen["player"]))
    _mapping(extra_player["player"])["unknown"] = 1
    with pytest.raises(ValueError, match="unknown field"):
        FairCombatObservation._from_payload(extra_player)
    monsters = _sequence(screen["monsters"])
    monster = dict(_mapping(monsters[0]))
    monster["intent"] = {"visibility": "visible", "category": "attack", "damage": None}
    null_damage = dict(screen)
    null_damage["monsters"] = [monster, *monsters[1:]]
    with pytest.raises(TypeError, match="omitted rather than null"):
        FairCombatObservation._from_payload(null_damage)
    invalid_intent = dict(screen)
    invalid_monster = dict(_mapping(monsters[0]))
    invalid_monster["intent"] = {"visibility": "visible", "category": "not-an-intent"}
    invalid_intent["monsters"] = [invalid_monster, *monsters[1:]]
    with pytest.raises(ValueError, match="invalid"):
        FairCombatObservation._from_payload(invalid_intent)


def test_fair_run_observation_schema_is_strict() -> None:
    payload = _run_observation_payload()
    parsed = FairRunObservation._from_payload(payload)
    assert parsed.schema_version == 1
    assert isinstance(parsed.context, FairRunContext)
    extra = dict(payload)
    extra["unknown"] = True
    with pytest.raises(ValueError, match="unknown field"):
        FairRunObservation._from_payload(extra)
    missing = dict(payload)
    missing.pop("screen")
    with pytest.raises(ValueError, match="missing"):
        FairRunObservation._from_payload(missing)
    null_screen = dict(payload)
    null_screen["screen"] = None
    with pytest.raises(TypeError, match="object"):
        FairRunObservation._from_payload(null_screen)
    wrong_schema = dict(payload)
    wrong_schema["schema_version"] = 2
    with pytest.raises(ValueError, match="unsupported fair run observation schema"):
        FairRunObservation._from_payload(wrong_schema)
    extra_context = dict(payload)
    extra_context["context"] = dict(_mapping(payload["context"]))
    _mapping(extra_context["context"])["unknown"] = 1
    with pytest.raises(ValueError, match="unknown field"):
        FairRunObservation._from_payload(extra_context)
    missing_context_field = dict(payload)
    missing_context = dict(_mapping(payload["context"]))
    missing_context.pop("player_hp")
    missing_context_field["context"] = missing_context
    with pytest.raises(ValueError, match="missing"):
        FairRunObservation._from_payload(missing_context_field)
    bool_gold = dict(payload)
    bool_gold["context"] = dict(_mapping(payload["context"]))
    _mapping(bool_gold["context"])["gold"] = True
    with pytest.raises(TypeError, match="integer"):
        FairRunObservation._from_payload(bool_gold)
    invalid_phase = dict(payload)
    invalid_phase["phase"] = "not-a-phase"
    with pytest.raises(ValueError, match="invalid"):
        FairRunObservation._from_payload(invalid_phase)
    mismatched = dict(payload)
    mismatched["phase"] = "combat"
    mismatched["kind"] = "map"
    with pytest.raises(ValueError, match="correspond"):
        FairRunObservation._from_payload(mismatched)
    complete_grid = dict(payload)
    complete_grid["phase"] = "complete"
    complete_grid["kind"] = "grid"
    with pytest.raises(ValueError, match="grid overlay"):
        FairRunObservation._from_payload(complete_grid)
    idle_combat = dict(payload)
    idle_combat["phase"] = "idle"
    idle_combat["kind"] = "combat"
    with pytest.raises(ValueError, match="idle phase"):
        FairRunObservation._from_payload(idle_combat)
