from __future__ import annotations

import json
from collections.abc import Callable
from typing import Literal, cast

import pytest
import torch

from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    COMBAT_PROXY_V1,
    CombatModelConfig,
    CombatOutcome,
    CombatRewardConfig,
    FairCombatPolicyValueNet,
    Vocabularies,
    VocabularyBuilder,
    puct_search_payload,
    rollout_puct_policy,
    select_puct_action,
)
from sts_sim.rl.puct import (
    FAIR_LEAF_BATCH_SCHEMA,
    PUCT_TEACHER_VERSION,
    network_leaf_evaluator,
    select_puct_action_with_evaluator,
    select_uniform_prior_constant_value_puct_action,
    select_uniform_prior_network_value_puct_action,
)
from sts_sim.rl.records import action_descriptor_from_payload, fair_observation_from_payload
from sts_sim.run import Decision


def _require_fair_leaf(item: dict[str, object]) -> None:
    fair_observation_from_payload(item["observation"])
    raw_choices = item["choices"]
    assert isinstance(raw_choices, list) and raw_choices
    for choice in raw_choices:
        action_descriptor_from_payload(choice)


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
    assert isinstance(item, dict)
    leaf = cast(dict[str, object], item)
    assert set(leaf) == {"observation", "choices"}
    _require_fair_leaf(leaf)
    choices = leaf["choices"]
    assert isinstance(choices, list) and choices
    return json.dumps(
        {
            "schema": FAIR_LEAF_BATCH_SCHEMA,
            "batch": [{"priors": [1.0] * len(choices), "value": 0.0}],
        }
    )


def _one_hot_evaluator(request_json: str) -> str:
    request = json.loads(request_json)
    choices = request["batch"][0]["choices"]
    priors = [0.0] * len(choices)
    priors[-1] = 1.0
    return json.dumps(
        {
            "schema": FAIR_LEAF_BATCH_SCHEMA,
            "batch": [{"priors": priors, "value": 0.5}],
        }
    )


def _proxy_outcome(
    status: Literal["won", "lost", "escaped", "truncated"],
    hp: int,
    max_hp: int,
    max_hp_change: int,
    gold_change: int,
    potions: int,
) -> CombatOutcome:
    return CombatOutcome(
        status,
        hp,
        max_hp,
        0,
        max_hp_change,
        gold_change,
        tuple("potion" for _ in range(potions)),
        (),
        True,
        False,
    )


def test_puct_payload_is_deterministic_and_budgeted() -> None:
    env = RunEnv.combat_fixture()
    first = puct_search_payload(env, _uniform_evaluator, simulation_budget=6, transition_budget=6)
    second = puct_search_payload(env, _uniform_evaluator, simulation_budget=6, transition_budget=6)
    assert first == second
    assert first["teacher_name"] == "privileged_puct"
    assert first["teacher_version"] == PUCT_TEACHER_VERSION
    assert first["transitions"] == 6
    assert first["completed_simulations"] == 6
    assert first["stop_reason"] == "simulation_budget"
    assert env.revision == RunEnv.combat_fixture().revision


def test_zero_c_puct_is_rejected_and_one_hot_priors_still_terminate() -> None:
    env = RunEnv.combat_fixture()
    with pytest.raises(ValueError, match="c_puct must be finite and positive"):
        puct_search_payload(
            env,
            _uniform_evaluator,
            c_puct=0.0,
            simulation_budget=16,
            transition_budget=100,
        )
    one_hot = puct_search_payload(
        env,
        _one_hot_evaluator,
        simulation_budget=16,
        transition_budget=100,
    )
    assert one_hot["completed_simulations"] == 16
    assert cast(int, one_hot["transitions"]) <= 16
    assert one_hot["stop_reason"] == "simulation_budget"
    visits = cast(list[int], one_hot["visits"])
    assert visits[-1] == max(visits)


def test_positive_budgets_are_required() -> None:
    env = RunEnv.combat_fixture()
    with pytest.raises(ValueError, match="simulation budget must be positive"):
        puct_search_payload(env, _uniform_evaluator, simulation_budget=0, transition_budget=1)
    with pytest.raises(ValueError, match="transition budget must be positive"):
        puct_search_payload(env, _uniform_evaluator, simulation_budget=1, transition_budget=0)


def test_puct_selects_original_sidecar_action() -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    selected = select_puct_action(
        env, decision, model, vocabularies, simulation_budget=4, transition_budget=4
    )
    assert any(candidate is selected for candidate in decision.actions)
    assert env.revision == decision.revision


def test_select_puct_action_requires_descriptor_alignment(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    import sts_sim.rl.puct as puct_module

    original = puct_module.puct_search_payload

    def misaligned(
        env: RunEnv,
        evaluator: Callable[[str], str],
        *,
        c_puct: float = 1.5,
        simulation_budget: int = 64,
        transition_budget: int = 64,
        reward_config: CombatRewardConfig | None = None,
        episode_root_max_hp: int | None = None,
        episode_root_gold: int | None = None,
        leaf_cache: str | None = None,
    ) -> dict[str, object]:
        payload = original(
            env,
            evaluator,
            c_puct=c_puct,
            simulation_budget=simulation_budget,
            transition_budget=transition_budget,
            reward_config=reward_config,
            episode_root_max_hp=episode_root_max_hp,
            episode_root_gold=episode_root_gold,
            leaf_cache=leaf_cache,
        )
        choices = list(cast(list[object], payload["choices"]))
        choices.reverse()
        payload["choices"] = choices
        payload["selected_index"] = 0
        return payload

    monkeypatch.setattr(puct_module, "puct_search_payload", misaligned)
    with pytest.raises(ValueError, match="not aligned with the public Decision"):
        select_puct_action(
            env, decision, model, vocabularies, simulation_budget=2, transition_budget=2
        )


def _install_leaf_cache_capture(
    monkeypatch: pytest.MonkeyPatch,
) -> list[str | None]:
    import sts_sim.rl.puct as puct_module

    captured: list[str | None] = []
    original = puct_module.puct_search_payload
    wrapped = cast("Callable[..., dict[str, object]]", original)

    def capture(*args: object, **kwargs: object) -> dict[str, object]:
        captured.append(cast(str | None, kwargs.get("leaf_cache")))
        return wrapped(*args, **kwargs)

    monkeypatch.setattr(puct_module, "puct_search_payload", capture)
    return captured


def test_select_puct_action_opts_into_exact_state_cache(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    captured = _install_leaf_cache_capture(monkeypatch)
    select_puct_action(env, decision, model, vocabularies, simulation_budget=2, transition_budget=2)
    assert captured == ["exact_state"]


def test_generic_evaluator_selector_defaults_to_cache_off(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()
    captured = _install_leaf_cache_capture(monkeypatch)
    selected = select_puct_action_with_evaluator(
        env, decision, _uniform_evaluator, simulation_budget=2, transition_budget=2
    )
    assert captured == [None]
    assert any(candidate is selected for candidate in decision.actions)


def test_deterministic_uniform_prior_wrappers_opt_into_exact_state_cache(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    captured = _install_leaf_cache_capture(monkeypatch)
    select_uniform_prior_network_value_puct_action(
        env, decision, model, vocabularies, simulation_budget=2, transition_budget=2
    )
    select_uniform_prior_constant_value_puct_action(
        env, decision, simulation_budget=2, transition_budget=2
    )
    assert captured == ["exact_state", "exact_state"]


def test_rollout_carries_public_episode_root_baselines(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    observation = decision.observation
    assert isinstance(observation, FairCombatObservation)
    captured: list[tuple[int | None, int | None]] = []

    def fake_select(*args: object, **kwargs: object) -> object:
        current = cast(Decision, args[1])
        captured.append(
            (
                cast(int | None, kwargs.get("episode_root_max_hp")),
                cast(int | None, kwargs.get("episode_root_gold")),
            )
        )
        return current.actions[0]

    monkeypatch.setattr("sts_sim.rl.puct.select_puct_action", fake_select)
    rollout_puct_policy(
        RunEnv.combat_fixture(),
        model,
        vocabularies,
        max_decisions=2,
        max_player_turns=8,
        simulation_budget=1,
        transition_budget=1,
    )
    assert captured
    assert all(item == captured[0] for item in captured)
    assert captured[0] == (observation.player.max_hp, observation.context.gold)


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
        simulation_budget=2,
        transition_budget=2,
    )
    assert truncated.status == "truncated"
    assert truncated.accepted_decisions == 1
    assert truncated.truncation_trigger == "accepted_decisions"

    def boom(_request_json: str) -> str:
        raise ValueError("injected evaluator failure")

    failed_env = RunEnv.combat_fixture()
    with pytest.raises(ValueError, match="injected evaluator failure"):
        puct_search_payload(failed_env, boom, simulation_budget=2, transition_budget=2)

    def fail_select(*_args: object, **_kwargs: object) -> None:
        raise ValueError("injected selector failure")

    monkeypatch.setattr("sts_sim.rl.puct.select_puct_action", fail_select)
    failed = rollout_puct_policy(
        RunEnv.combat_fixture(),
        model,
        vocabularies,
        max_decisions=8,
        max_player_turns=8,
        simulation_budget=1,
        transition_budget=1,
    )
    assert failed.status == "error"
    assert failed.error == "injected selector failure"


def test_malformed_evaluator_is_rejected() -> None:
    env = RunEnv.combat_fixture()

    def missing_schema(request_json: str) -> str:
        del request_json
        return json.dumps({"batch": [{"priors": [1.0], "value": 0.0}]})

    with pytest.raises(RuntimeError, match="missing field `schema`"):
        puct_search_payload(env, missing_schema, simulation_budget=1, transition_budget=1)

    def bad_value(request_json: str) -> str:
        request = json.loads(request_json)
        count = len(request["batch"][0]["choices"])
        return json.dumps(
            {
                "schema": FAIR_LEAF_BATCH_SCHEMA,
                "batch": [{"priors": [1.0] * count, "value": 2.0}],
            }
        )

    with pytest.raises(ValueError):
        puct_search_payload(env, bad_value, simulation_budget=1, transition_budget=1)


def test_network_leaf_evaluator_runs_through_native_puct() -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    payload = puct_search_payload(
        env,
        network_leaf_evaluator(model, vocabularies),
        simulation_budget=2,
        transition_budget=2,
    )
    index = payload["selected_index"]
    assert type(index) is int
    assert 0 <= index < len(decision.actions)
    assert len(cast(list[object], payload["visits"])) == len(decision.actions)
    leaf_evaluations = payload["leaf_evaluations"]
    assert type(leaf_evaluations) is int
    assert leaf_evaluations >= 1


def test_rust_proxy_matches_python_combat_proxy_v1() -> None:
    lost = COMBAT_PROXY_V1.value(_proxy_outcome("lost", 0, 80, 0, 0, 0))
    escaped = COMBAT_PROXY_V1.value(_proxy_outcome("escaped", 40, 80, 0, 0, 0))
    won = COMBAT_PROXY_V1.value(_proxy_outcome("won", 80, 80, 0, 0, 0))
    won_resources = COMBAT_PROXY_V1.value(_proxy_outcome("won", 40, 80, 10, 100, 2))
    current_root = COMBAT_PROXY_V1.value(_proxy_outcome("won", 40, 80, 0, 0, 0))
    episode_root = COMBAT_PROXY_V1.value(_proxy_outcome("won", 40, 80, 10, 49, 0))
    assert lost == -1.0
    assert escaped == pytest.approx(0.35)
    assert won == pytest.approx(0.95)
    assert won_resources == pytest.approx(0.89)
    assert current_root == pytest.approx(0.85)
    assert episode_root is not None and current_root is not None
    assert episode_root > current_root


def test_empty_leaf_cache_is_rejected_by_native_search() -> None:
    env = RunEnv.combat_fixture()
    with pytest.raises(ValueError, match="empty"):
        env.puct_search_payload(
            _uniform_evaluator,
            simulation_budget=1,
            transition_budget=1,
            leaf_cache="",
        )


def test_empty_reward_json_is_rejected() -> None:
    env = RunEnv.combat_fixture()
    with pytest.raises(ValueError, match="reward config JSON must be a nonempty"):
        env.puct_search_payload(
            _uniform_evaluator,
            simulation_budget=1,
            transition_budget=1,
            reward_config_json="",
        )


def test_transition_budget_stop_reason_is_reported() -> None:
    env = RunEnv.combat_fixture()
    payload = puct_search_payload(
        env,
        _uniform_evaluator,
        simulation_budget=8,
        transition_budget=3,
    )
    assert payload["transitions"] == 3
    assert payload["completed_simulations"] == 3
    assert payload["stop_reason"] == "transition_budget"
