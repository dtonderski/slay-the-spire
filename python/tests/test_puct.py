from __future__ import annotations

import json
from typing import cast

import pytest
import torch

from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    CombatModelConfig,
    FairCombatPolicyValueNet,
    Vocabularies,
    VocabularyBuilder,
    puct_search_payload,
    rollout_puct_policy,
    select_puct_action,
)
from sts_sim.rl.puct import FAIR_LEAF_BATCH_SCHEMA, network_leaf_evaluator


def _tiny_policy_net() -> tuple[RunEnv, FairCombatPolicyValueNet, Vocabularies]:
    env = RunEnv.combat_fixture()
    decision = env.decision()
    observation = decision.observation
    assert isinstance(observation, FairCombatObservation)
    descriptors = tuple(action.descriptor() for action in decision.actions)
    builder = VocabularyBuilder()
    builder.add(observation, descriptors)
    vocabularies = builder.freeze()
    torch.manual_seed(7)
    model = FairCombatPolicyValueNet(
        vocabularies,
        CombatModelConfig(width=32, heads=4, layers=1, feedforward_width=64),
    ).eval()
    return env, model, vocabularies


def _uniform_evaluator(request_json: str) -> str:
    request = json.loads(request_json)
    assert request["schema"] == FAIR_LEAF_BATCH_SCHEMA
    assert set(request) == {"schema", "batch"}
    batch = request["batch"]
    assert isinstance(batch, list) and len(batch) == 1
    item = batch[0]
    assert set(item) == {"observation", "choices"}
    encoded = json.dumps(item)
    for field in ("card_id", "monster_id", "content_id", "rng"):
        assert field not in encoded
    choices = item["choices"]
    assert isinstance(choices, list) and choices
    return json.dumps(
        {
            "schema": FAIR_LEAF_BATCH_SCHEMA,
            "batch": [{"priors": [1.0] * len(choices), "value": 0.0}],
        }
    )


def test_puct_payload_is_deterministic_and_budgeted() -> None:
    env = RunEnv.combat_fixture()
    first = puct_search_payload(env, _uniform_evaluator, transition_budget=6)
    second = puct_search_payload(env, _uniform_evaluator, transition_budget=6)
    assert first == second
    assert first["teacher_name"] == "privileged_puct"
    assert first["transitions"] == 6
    assert first["completed_simulations"] == 6
    assert first["budget_exhausted"] is True
    assert env.revision == RunEnv.combat_fixture().revision


def test_puct_selects_original_sidecar_action() -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    selected = select_puct_action(env, decision, model, vocabularies, transition_budget=4)
    assert any(candidate is selected for candidate in decision.actions)
    assert env.revision == decision.revision


def test_puct_episode_respects_caps_and_keeps_errors_in_status(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    truncated = rollout_puct_policy(
        env,
        model,
        vocabularies,
        max_decisions=1,
        max_player_turns=100,
        transition_budget=2,
    )
    assert truncated.status == "truncated"
    assert truncated.accepted_decisions == 1
    assert truncated.truncation_trigger == "accepted_decisions"

    def boom(_request_json: str) -> str:
        raise ValueError("injected evaluator failure")

    failed_env = RunEnv.combat_fixture()
    with pytest.raises(RuntimeError, match="injected evaluator failure"):
        puct_search_payload(failed_env, boom, transition_budget=2)

    def fail_select(*_args: object, **_kwargs: object) -> None:
        raise ValueError("injected selector failure")

    monkeypatch.setattr("sts_sim.rl.puct.select_puct_action", fail_select)
    failed = rollout_puct_policy(
        RunEnv.combat_fixture(),
        model,
        vocabularies,
        max_decisions=8,
        max_player_turns=8,
        transition_budget=1,
    )
    assert failed.status == "error"
    assert failed.error == "injected selector failure"


def test_malformed_evaluator_is_rejected() -> None:
    env = RunEnv.combat_fixture()

    def wrong_length(request_json: str) -> str:
        del request_json
        return json.dumps({"batch": [{"priors": [1.0], "value": 0.0}]})

    with pytest.raises(RuntimeError):
        puct_search_payload(env, wrong_length, transition_budget=1)

    def bad_value(request_json: str) -> str:
        request = json.loads(request_json)
        count = len(request["batch"][0]["choices"])
        return json.dumps({"batch": [{"priors": [1.0] * count, "value": 2.0}]})

    with pytest.raises(RuntimeError):
        puct_search_payload(env, bad_value, transition_budget=1)


def test_network_leaf_evaluator_runs_through_native_puct() -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    payload = puct_search_payload(
        env,
        network_leaf_evaluator(model, vocabularies),
        transition_budget=2,
    )
    assert type(payload["selected_index"]) is int
    assert 0 <= cast(int, payload["selected_index"]) < len(decision.actions)
    assert len(cast(list[object], payload["visits"])) == len(decision.actions)
    assert payload["unique_evaluations"] >= 1
