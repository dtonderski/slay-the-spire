from __future__ import annotations

from pathlib import Path

from sts_sim import UnknownPublicContentError
from sts_sim.rl import data as data_module
from sts_sim.rl import generate_legal_roots

_EVENT_COMBAT_LOSS_SEEDS = (
    "PUCTSCALEV21894",
    "PUCTSCALEV22459",
    "PUCTSCALEV25859",
    "PUCTSCALEV26249",
    "PUCTSCALEV26852",
)
_PRISMATIC_REWARD_SEED = "PUCTSCALEV22838"
_EVENT_LETHAL_HP_SEED = "PUCTSCALEV21131"


def test_event_combat_loss_proceed_is_terminal_run_not_generation_error(tmp_path: Path) -> None:
    manifest = generate_legal_roots(
        tmp_path / "event-loss",
        list(_EVENT_COMBAT_LOSS_SEEDS),
        max_run_steps=2048,
        combat_depth=4,
    )
    by_seed = {exclusion.source_seed: exclusion for exclusion in manifest.exclusions}
    assert set(by_seed) == set(_EVENT_COMBAT_LOSS_SEEDS)
    for seed in _EVENT_COMBAT_LOSS_SEEDS:
        exclusion = by_seed[seed]
        assert exclusion.reason == "terminal_run"
        assert "of requested depth 4" in exclusion.detail
    assert all(
        "public run action is invalid" not in exclusion.detail for exclusion in manifest.exclusions
    )
    assert manifest.roots == ()


def test_prismatic_unmodeled_reward_card_is_typed_exclusion(tmp_path: Path) -> None:
    manifest = generate_legal_roots(
        tmp_path / "prismatic",
        [_PRISMATIC_REWARD_SEED],
        max_run_steps=2048,
        combat_depth=4,
    )
    accounted = {seed for root in manifest.roots for seed in root.source_seeds}
    accounted.update(exclusion.source_seed for exclusion in manifest.exclusions)
    assert accounted == {_PRISMATIC_REWARD_SEED}
    if manifest.roots:
        return
    assert len(manifest.exclusions) == 1
    exclusion = manifest.exclusions[0]
    assert exclusion.reason == "unmodeled_public_content"
    assert exclusion.detail
    assert not exclusion.detail.isdigit()
    assert "public combat content is unknown" not in exclusion.detail


def test_lethal_event_hp_is_terminal_run_not_dead_player_combat(tmp_path: Path) -> None:
    manifest = generate_legal_roots(
        tmp_path / "event-lethal",
        [_EVENT_LETHAL_HP_SEED],
        max_run_steps=2048,
        combat_depth=4,
    )
    assert len(manifest.exclusions) == 1
    exclusion = manifest.exclusions[0]
    assert exclusion.source_seed == _EVENT_LETHAL_HP_SEED
    assert exclusion.reason == "terminal_run"
    assert "of requested depth 4" in exclusion.detail
    assert manifest.roots == ()


def test_unmodeled_public_content_exclusion_uses_public_key_only() -> None:
    modeled = data_module._unmodeled_public_content_exclusion(
        "SEED",
        UnknownPublicContentError("public combat content is unmodeled: FLYING_KNEE"),
    )
    assert modeled.reason == "unmodeled_public_content"
    assert modeled.detail == "FLYING_KNEE"
    assert not any(character.isdigit() for character in modeled.detail)

    unknown = data_module._unmodeled_public_content_exclusion(
        "SEED",
        UnknownPublicContentError("public combat content is unknown"),
    )
    assert unknown.reason == "unmodeled_public_content"
    assert unknown.detail == "unknown public identity"
    assert "9999999" not in unknown.detail

    leaked = data_module._unmodeled_public_content_exclusion(
        "SEED",
        UnknownPublicContentError("public combat content is unmodeled: 9999999"),
    )
    assert leaked.reason == "unmodeled_public_content"
    assert leaked.detail == "unmodeled public content"
    assert "9999999" not in leaked.detail
