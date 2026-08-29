import ast
from pathlib import Path
from typing import cast

import pytest

import sts_sim.content as content_module
from sts_sim import CARD_CATALOGUE, CARD_DEFINITIONS, Card, CardDefinition, _native


def test_card_catalogue_is_complete_sorted_unique_and_round_trips_to_enum() -> None:
    keys = tuple(definition.content_key for definition in CARD_CATALOGUE)

    assert len(CARD_CATALOGUE) == 251
    assert keys == tuple(sorted(keys))
    assert len(set(keys)) == len(keys)
    assert keys == tuple(CARD_DEFINITIONS)
    assert set(keys) == {card.value for card in Card}
    assert tuple(CARD_DEFINITIONS.values()) == CARD_CATALOGUE


def test_card_enum_preserves_native_catalogue_iteration_order() -> None:
    assert tuple(card.value for card in Card) == tuple(_native.card_keys())


def test_card_stub_members_cover_every_runtime_member() -> None:
    stub_path = Path(content_module.__file__).with_suffix(".pyi")
    syntax = ast.parse(stub_path.read_text(encoding="utf-8"), filename=str(stub_path))
    card_class = next(
        node
        for node in syntax.body
        if isinstance(node, ast.ClassDef) and node.name == "Card"
    )
    stub_members = {
        statement.target.id
        for statement in card_class.body
        if isinstance(statement, ast.AnnAssign)
        and isinstance(statement.target, ast.Name)
    }

    assert stub_members == set(Card.__members__)


def test_card_catalogue_exposes_existing_authoritative_metadata() -> None:
    bash = CARD_DEFINITIONS["Bash"]
    assert bash.display_name == "Bash"
    assert bash.printed_cost == 2
    assert bash.card_type == "attack"
    assert bash.rarity == "common"
    assert bash.target == "enemy"
    assert bash.values.damage == 8
    assert bash.values.vulnerable == 2
    assert not bash.keywords.exhaust

    dramatic_entrance = CARD_DEFINITIONS["Dramatic Entrance"]
    assert dramatic_entrance.keywords.innate
    assert dramatic_entrance.keywords.exhaust


def test_curses_are_explicit_without_adding_a_curse_card_type() -> None:
    parasite = CARD_DEFINITIONS["Parasite"]
    wound = CARD_DEFINITIONS["Wound"]

    assert parasite.is_curse
    assert parasite.card_type == "status"
    assert not wound.is_curse
    assert wound.card_type == "status"


def test_card_catalogue_mapping_is_immutable() -> None:
    mutable_view = cast(dict[str, CardDefinition], CARD_DEFINITIONS)
    with pytest.raises(TypeError):
        mutable_view["Bash"] = CARD_DEFINITIONS["Strike_R"]
