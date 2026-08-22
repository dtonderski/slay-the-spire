import json
from typing import cast

import pytest

import sts_sim
from sts_sim import (
    Card,
    FairCardDynamicValues,
    FairCombatObservation,
    FairOrb,
    FairRunObservation,
    Potion,
    Relic,
    RunEnv,
    StaleDecisionError,
)


def test_package_root_exposes_one_environment_and_action_concept() -> None:
    assert sts_sim.RunEnv is RunEnv
    assert hasattr(sts_sim, "Action")
    assert not hasattr(sts_sim, "OmniRunEnv")
    assert not hasattr(sts_sim, "FairCombatEnv")
    assert not hasattr(sts_sim, "ExactRunAction")
    assert not hasattr(sts_sim, "PlayerChoice")


def test_run_env_has_one_action_list_and_one_step_method() -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()

    assert isinstance(decision.observation, FairCombatObservation)
    assert decision.actions == env.legal_actions()
    assert all(type(action).__name__ == "Action" for action in decision.actions)

    end_turn = next(action for action in decision.actions if action.kind == "end_turn")
    result = env.step(end_turn)

    assert result.decision.revision == 1
    assert isinstance(result.decision.observation, FairCombatObservation)


def test_schema_two_orbs_and_windmill_dynamic_value_are_typed() -> None:
    env = RunEnv.combat_fixture()
    state = env.full_state()
    combat = cast(dict[str, object], state["combat"])
    combat["max_orbs"] = 3
    combat["orbs"] = ["Lightning", {"Dark": {"evoke": 24}}]

    projected = RunEnv.from_state_json_for_debugging(json.dumps(state)).observation()

    assert isinstance(projected, FairCombatObservation)
    assert projected.schema_version == 2
    assert projected.orb_slots[0].orb == FairOrb(type="lightning", evoke=None)
    assert projected.orb_slots[1].orb == FairOrb(type="dark", evoke=24)
    assert projected.orb_slots[2].orb is None
    assert (
        FairCardDynamicValues._from_payload({"windmill_retain_damage": 8})
        .windmill_retain_damage
        == 8
    )
    assert (
        FairCardDynamicValues._from_payload({}).windmill_retain_damage is None
    ), "stored V1 card payloads remain readable"


def test_action_from_old_decision_is_stale() -> None:
    env = RunEnv.combat_fixture()
    action = next(action for action in env.legal_actions() if action.kind == "end_turn")

    env.step(action)

    with pytest.raises(StaleDecisionError, match="public run decision is stale"):
        env.step(action)


def test_clone_can_evaluate_the_same_action_without_mutating_parent() -> None:
    env = RunEnv.combat_fixture()
    clone = env.clone()
    action = env.legal_actions()[0]
    parent_snapshot = env.snapshot()

    clone.step(action)

    assert env.snapshot().hash == parent_snapshot.hash
    assert clone.snapshot().hash != parent_snapshot.hash


def test_full_state_and_snapshot_are_distinct_projections() -> None:
    env = RunEnv.combat_fixture()

    full_state = env.full_state()
    snapshot = env.snapshot()
    restored = RunEnv.from_snapshot(snapshot)

    assert full_state["phase"] == "Combat"
    assert snapshot.json != ""
    assert restored.snapshot().hash == snapshot.hash
    assert restored.revision == 0


def test_new_run_has_a_fair_event_observation_and_the_same_action_type() -> None:
    env = RunEnv.new_ironclad("ABC123", ascension=0)
    decision = env.decision()

    assert isinstance(decision.observation, FairRunObservation)
    assert decision.actions
    assert all(type(action).__name__ == "Action" for action in decision.actions)
    observation = env.observation()
    assert isinstance(observation, FairRunObservation)
    assert observation.kind == "event"
    assert observation.context.player_hp is not None
    assert observation.context.player_max_hp is not None
    assert observation.context.deck
    assert observation.context.relics
    assert observation.context.potion_slots


def test_noncombat_actions_have_public_screen_slots() -> None:
    event = RunEnv.new_ironclad("ABC123").decision()
    assert isinstance(event.observation, FairRunObservation)
    assert event.actions[0].kind == "event_choose"
    assert event.actions[0].option_slot == 0
    assert event.observation.screen["choices"]

    map_decision = RunEnv.map_fixture().decision()
    assert isinstance(map_decision.observation, FairRunObservation)
    assert all(action.kind == "choose_map_node" for action in map_decision.actions)
    assert [action.node_slot for action in map_decision.actions] == [1, 2]
    assert map_decision.observation.screen["reachable_nodes"] == [1, 2]


def test_action_text_is_compact_for_printing_and_notebooks() -> None:
    decision = RunEnv.new_ironclad("ABC123").decision()
    action = decision.actions[0]

    assert str(action) == "event_choose(option=0, revision=0)"
    assert repr(action) == "Action(event_choose(option=0, revision=0))"
    assert "VAction" not in repr(decision.actions)
    assert "hand_slot=None" not in repr(decision.actions)


def test_typed_debug_content_helpers_mutate_the_run_and_revision() -> None:
    env = RunEnv.new_ironclad("typeddebug")
    old_action = env.decision().actions[0]

    env.add_card(Card.BASH)
    env.add_relic(Relic.INK_BOTTLE)
    env.add_potion(Potion.FIRE)

    observation = env.observation()
    assert isinstance(observation, FairRunObservation)
    assert any(card.content_key == "Bash" for card in observation.context.deck)
    assert "Ink Bottle" in observation.context.relics
    assert observation.context.potion_slots[0].content_key == "fire"
    assert env.revision == 3

    with pytest.raises(StaleDecisionError, match="public run decision is stale"):
        env.step(old_action)


def test_content_catalogues_are_complete_python_enums() -> None:
    assert len(Card) == 245
    assert len(Relic) == 157
    assert len(Potion) == 33
    assert Card.BASH.value == "Bash"
    assert Relic.INK_BOTTLE.value == "Ink Bottle"
    assert Potion.GAMBLERS_BREW.value == "GamblersBrew"
