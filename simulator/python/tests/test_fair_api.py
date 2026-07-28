import pytest

from sts_sim import InvalidChoiceError, StaleDecisionError
from sts_sim.fair import FairCombatEnv, PlayerChoice, PlayerChoiceRequest


def test_fair_decision_is_typed_and_atomic() -> None:
    env = FairCombatEnv.combat_fixture()
    decision = env.decision()

    assert isinstance(decision.observation.hand, tuple)
    assert decision.decision_revision == 0
    assert decision.observation.hand
    assert all(choice.kind for choice in decision.choices)


def test_fair_step_advances_revision_without_exposing_snapshot() -> None:
    env = FairCombatEnv.combat_fixture()
    decision = env.decision()
    choice = next(choice for choice in decision.choices if choice.kind == "end_turn")

    result = env.step(PlayerChoiceRequest(decision.decision_revision, choice))

    assert not result.terminal
    assert result.decision is not None
    assert result.decision.decision_revision == 1
    assert not hasattr(env, "state_json")


def test_fair_step_rejects_stale_revision() -> None:
    env = FairCombatEnv.combat_fixture()
    decision = env.decision()
    choice = next(choice for choice in decision.choices if choice.kind == "end_turn")

    try:
        env.step(PlayerChoiceRequest(decision.decision_revision + 1, choice))
    except StaleDecisionError as error:
        assert str(error) == "public combat decision is stale"
    else:
        raise AssertionError("stale public choice must be rejected")


def test_fair_step_rejects_invalid_public_choice() -> None:
    env = FairCombatEnv.combat_fixture()
    decision = env.decision()

    with pytest.raises(InvalidChoiceError, match="public combat choice is invalid"):
        env.step(PlayerChoiceRequest(decision.decision_revision, PlayerChoice.play_hand_slot(999)))
