from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Literal, cast

import pytest
import torch

from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    MATCHED_PUCT_REPORT_ARMS,
    MATCHED_PUCT_SEARCH_ARMS,
    CombatModelConfig,
    FairCombatPolicyValueNet,
    PolicyValueOutput,
    Vocabularies,
    VocabularyBuilder,
    evaluate_matched_puct_gameplay,
    evaluate_matched_puct_roots,
    gameplay,
    puct_search_payload,
    select_puct_action,
    select_uniform_prior_constant_value_puct_action,
    select_uniform_prior_network_value_puct_action,
)
from sts_sim.rl.puct import (
    constant_value_uniform_prior_leaf_evaluator,
    network_leaf_evaluator,
    uniform_prior_network_value_leaf_evaluator,
)
from sts_sim.rl.tensor import BatchedCombatDecision

_PREDECLARATION = (
    Path(__file__).resolve().parents[2]
    / "docs"
    / "puct-teacher-control-v1"
    / "next-epoch-control-predeclaration.json"
)
LogitOverride = Literal["keep", "nan"] | int
ValueOverride = Literal["keep", "nan", "action_count", "neg_action_count"]


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


def _capture_leaf_request(env: RunEnv) -> str:
    captured: list[str] = []

    def capture(request_json: str) -> str:
        captured.append(request_json)
        return constant_value_uniform_prior_leaf_evaluator()(request_json)

    puct_search_payload(env, capture, simulation_budget=1, transition_budget=1)
    assert captured
    return captured[0]


def _install_output_override(
    monkeypatch: pytest.MonkeyPatch,
    model: FairCombatPolicyValueNet,
    *,
    logits: LogitOverride = "keep",
    value: ValueOverride = "keep",
) -> None:
    original = model.forward

    def forward(batch: BatchedCombatDecision) -> PolicyValueOutput:
        out = original(batch)
        new_logits = out.logits
        new_value = out.value
        if logits == "nan":
            poisoned = torch.full_like(out.logits, float("nan"))
            new_logits = poisoned.masked_fill(~batch.action_mask, float("-inf"))
        elif type(logits) is int:
            peaked = torch.full_like(out.logits, -80.0)
            peaked = peaked.masked_fill(~batch.action_mask, float("-inf"))
            if logits >= 0:
                peaked[..., logits] = 80.0
            else:
                last = (batch.action_mask.sum(dim=-1) - 1).long()
                peaked[torch.arange(peaked.shape[0], device=peaked.device), last] = 80.0
            new_logits = peaked
        if value == "nan":
            new_value = torch.full_like(out.value, float("nan"))
        elif value == "action_count":
            legal = batch.action_mask.sum(dim=-1).to(dtype=out.value.dtype)
            new_value = torch.tanh(legal - 4.0)
        elif value == "neg_action_count":
            legal = batch.action_mask.sum(dim=-1).to(dtype=out.value.dtype)
            new_value = torch.tanh(4.0 - legal)
        return PolicyValueOutput(new_logits, new_value, out.entity_states)

    monkeypatch.setattr(model, "forward", forward)


def _evaluate_one_root(
    root_id: str,
    snapshot_bytes: bytes,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
) -> dict[str, object]:
    return evaluate_matched_puct_roots(
        split_roots=((root_id, snapshot_bytes),),
        evaluation_seed=0,
        model=model,
        vocabularies=vocabularies,
        transition_budget=8,
        simulation_budget=8,
        c_puct=1.5,
        beam_depth=2,
        beam_width=4,
        max_decisions=1,
        max_player_turns=100,
        deduplicate_search_states=True,
    )


def test_report_arm_names_match_the_predeclaration() -> None:
    payload = json.loads(_PREDECLARATION.read_text(encoding="utf-8"))
    assert set(MATCHED_PUCT_REPORT_ARMS) == set(payload["search_arms"])
    assert set(MATCHED_PUCT_SEARCH_ARMS) == set(payload["search_arms"])
    ablation = MATCHED_PUCT_SEARCH_ARMS["uniform_prior_network_value_puct"]
    unguided = MATCHED_PUCT_SEARCH_ARMS["uniform_prior_constant_value_puct"]
    assert ablation["role"] == "policy-prior ablation, not an unguided baseline"
    assert ablation["uses_checkpoint"] is True
    assert unguided["role"] == "equal-budget unguided-search arm"
    assert unguided["uses_checkpoint"] is False
    unguided_leaf = cast(dict[str, object], unguided["leaf_value"])
    assert unguided_leaf["value"] == 0.0


def test_uniform_prior_evaluators_ignore_poisoned_policy_logits(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    request = _capture_leaf_request(env)
    choice_count = len(json.loads(request)["batch"][0]["choices"])
    expected = [1.0 / choice_count] * choice_count
    _install_output_override(monkeypatch, model, logits=0)
    learned = json.loads(network_leaf_evaluator(model, vocabularies)(request))
    assert learned["batch"][0]["priors"][0] == pytest.approx(1.0)
    assert max(learned["batch"][0]["priors"][1:]) == pytest.approx(0.0)
    uniform_peaked = json.loads(
        uniform_prior_network_value_leaf_evaluator(model, vocabularies)(request)
    )
    assert uniform_peaked["batch"][0]["priors"] == expected
    _install_output_override(monkeypatch, model, logits="nan")
    with pytest.raises(ValueError, match="logits are not finite"):
        network_leaf_evaluator(model, vocabularies)(request)
    uniform = json.loads(uniform_prior_network_value_leaf_evaluator(model, vocabularies)(request))
    constant = json.loads(constant_value_uniform_prior_leaf_evaluator()(request))
    assert uniform["batch"][0]["priors"] == expected
    assert constant["batch"][0]["priors"] == expected
    assert constant["batch"][0]["value"] == 0.0


def test_constant_value_evaluator_ignores_poisoned_network_values(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    request = _capture_leaf_request(env)
    _install_output_override(monkeypatch, model, value="nan")
    with pytest.raises(ValueError, match="value must be finite"):
        network_leaf_evaluator(model, vocabularies)(request)
    with pytest.raises(ValueError, match="value must be finite"):
        uniform_prior_network_value_leaf_evaluator(model, vocabularies)(request)
    constant = json.loads(constant_value_uniform_prior_leaf_evaluator()(request))
    assert constant["batch"][0]["value"] == 0.0


def test_constant_value_search_never_evaluates_the_checkpoint(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    calls = {"count": 0}
    original = model.forward

    def counting(batch: BatchedCombatDecision) -> PolicyValueOutput:
        calls["count"] += 1
        return original(batch)

    monkeypatch.setattr(model, "forward", counting)
    constant = puct_search_payload(
        env,
        constant_value_uniform_prior_leaf_evaluator(),
        simulation_budget=8,
        transition_budget=8,
    )
    assert calls["count"] == 0
    network_value = puct_search_payload(
        env,
        uniform_prior_network_value_leaf_evaluator(model, vocabularies),
        simulation_budget=8,
        transition_budget=8,
    )
    assert calls["count"] > 0
    assert type(constant["selected_index"]) is int
    assert type(network_value["selected_index"]) is int
    decision = env.decision()
    selected = select_uniform_prior_constant_value_puct_action(
        env, decision, simulation_budget=4, transition_budget=4
    )
    assert any(candidate is selected for candidate in decision.actions)


def test_uniform_prior_search_ignores_peaked_policy_logits(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    assert len(decision.actions) >= 2
    _install_output_override(monkeypatch, model, logits=0)
    learned_first = puct_search_payload(
        env,
        network_leaf_evaluator(model, vocabularies),
        simulation_budget=16,
        transition_budget=16,
    )
    uniform_first = puct_search_payload(
        env,
        uniform_prior_network_value_leaf_evaluator(model, vocabularies),
        simulation_budget=16,
        transition_budget=16,
    )
    constant_first = puct_search_payload(
        env,
        constant_value_uniform_prior_leaf_evaluator(),
        simulation_budget=16,
        transition_budget=16,
    )
    _install_output_override(monkeypatch, model, logits=-1)
    learned_last = puct_search_payload(
        env,
        network_leaf_evaluator(model, vocabularies),
        simulation_budget=16,
        transition_budget=16,
    )
    uniform_last = puct_search_payload(
        env,
        uniform_prior_network_value_leaf_evaluator(model, vocabularies),
        simulation_budget=16,
        transition_budget=16,
    )
    constant_last = puct_search_payload(
        env,
        constant_value_uniform_prior_leaf_evaluator(),
        simulation_budget=16,
        transition_budget=16,
    )
    assert learned_first["selected_index"] != learned_last["selected_index"]
    assert uniform_first["selected_index"] == uniform_last["selected_index"]
    assert uniform_first["visits"] == uniform_last["visits"]
    assert constant_first["selected_index"] == constant_last["selected_index"]
    assert constant_first["visits"] == constant_last["visits"]
    first = select_uniform_prior_network_value_puct_action(
        env, decision, model, vocabularies, simulation_budget=4, transition_budget=4
    )
    assert any(candidate is first for candidate in decision.actions)
    learned = select_puct_action(
        env, decision, model, vocabularies, simulation_budget=4, transition_budget=4
    )
    assert any(candidate is learned for candidate in decision.actions)


def test_constant_value_search_ignores_state_dependent_network_values(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    _install_output_override(monkeypatch, model, value="action_count")
    network_plus = puct_search_payload(
        env,
        uniform_prior_network_value_leaf_evaluator(model, vocabularies),
        simulation_budget=16,
        transition_budget=16,
    )
    constant_plus = puct_search_payload(
        env,
        constant_value_uniform_prior_leaf_evaluator(),
        simulation_budget=16,
        transition_budget=16,
    )
    _install_output_override(monkeypatch, model, value="neg_action_count")
    network_minus = puct_search_payload(
        env,
        uniform_prior_network_value_leaf_evaluator(model, vocabularies),
        simulation_budget=16,
        transition_budget=16,
    )
    constant_minus = puct_search_payload(
        env,
        constant_value_uniform_prior_leaf_evaluator(),
        simulation_budget=16,
        transition_budget=16,
    )
    assert network_plus["value"] != network_minus["value"]
    assert constant_plus["value"] == constant_minus["value"]
    assert constant_plus["selected_index"] == constant_minus["selected_index"]
    assert constant_plus["visits"] == constant_minus["visits"]


def test_six_arms_restore_identical_root_bytes_independently(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    snapshot = env.snapshot()
    snapshot_bytes = snapshot.json.encode()
    root_id = hashlib.sha256(snapshot_bytes).hexdigest()
    restores: list[bytes] = []
    original = gameplay._restore_independently

    def spy(snapshot: bytes, identity: str) -> RunEnv:
        restores.append(snapshot)
        return original(snapshot, identity)

    monkeypatch.setattr(gameplay, "_restore_independently", spy)
    report = _evaluate_one_root(root_id, snapshot_bytes, model, vocabularies)
    assert len(restores) == 12
    assert all(payload == snapshot_bytes for payload in restores)
    policies = cast(
        dict[str, object], cast(list[dict[str, object]], report["per_root"])[0]["policies"]
    )
    assert tuple(policies) == MATCHED_PUCT_REPORT_ARMS
    search_arms = cast(dict[str, dict[str, object]], report["search_arms"])
    assert search_arms["uniform_prior_network_value_puct"]["role"] == (
        "policy-prior ablation, not an unguided baseline"
    )


def test_uniform_prior_arm_errors_stay_in_the_official_denominator(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    snapshot = env.snapshot()
    snapshot_bytes = snapshot.json.encode()
    root_id = hashlib.sha256(snapshot_bytes).hexdigest()

    def boom(*_args: object, **_kwargs: object) -> None:
        raise OverflowError("injected uniform overflow")

    monkeypatch.setattr("sts_sim.rl.puct.rollout_uniform_prior_constant_value_puct_policy", boom)
    report = _evaluate_one_root(root_id, snapshot_bytes, model, vocabularies)
    policies = cast(
        dict[str, dict[str, object]],
        cast(list[dict[str, object]], report["per_root"])[0]["policies"],
    )
    row = policies["uniform_prior_constant_value_puct"]
    assert row["status"] == "error"
    assert row["error"] == "injected uniform overflow"
    aggregates = cast(dict[str, dict[str, object]], report["aggregates"])
    arm = aggregates["uniform_prior_constant_value_puct"]
    assert arm["errors"] == 1
    assert arm["win_denominator"] == 1


def test_six_policy_gameplay_rejects_nonpositive_budgets() -> None:
    missing = Path("missing-roots.json")
    checkpoint = Path("missing-checkpoint.pt")
    with pytest.raises(ValueError, match="c_puct must be finite and positive"):
        evaluate_matched_puct_gameplay(missing, checkpoint, c_puct=float("inf"))
    with pytest.raises(ValueError, match="simulation_budget must be a positive integer"):
        evaluate_matched_puct_gameplay(missing, checkpoint, simulation_budget=-1)
