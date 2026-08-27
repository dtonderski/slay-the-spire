from __future__ import annotations

import ast
import hashlib
import json
import os
import subprocess
import sys
from collections.abc import Mapping
from dataclasses import dataclass, fields, replace
from pathlib import Path
from typing import cast, get_args

import pytest
import torch

from sts_sim import (
    ActionDescriptor,
    FairCombatObservation,
    FairCounter,
    FairOrb,
    FairOrbSlot,
    FairPile,
    FairPower,
    FairRelic,
    FairSelection,
    FairSelectionOption,
    RunEnv,
    _native,
)
from sts_sim.fair import PlayerChoiceKind
from sts_sim.rl import (
    FIELD_COVERAGE,
    SCALAR_INDEX,
    FrozenVocabulary,
    TensorizedCombatDecision,
    Vocabularies,
    VocabularyBuilder,
    collate_combat_tensors,
    field_coverage_mismatches,
    tensorize_combat,
)
from sts_sim.rl import tensor as tensor_module


@dataclass(frozen=True)
class SyntheticLeaf:
    visible_value: int


@dataclass(frozen=True)
class SyntheticRoot:
    leaf: SyntheticLeaf


def _decision() -> tuple[FairCombatObservation, tuple[ActionDescriptor, ...]]:
    decision = RunEnv.combat_fixture().decision()
    assert isinstance(decision.observation, FairCombatObservation)
    return decision.observation, tuple(action.descriptor() for action in decision.actions)


def _vocab(*records: tuple[FairCombatObservation, tuple[ActionDescriptor, ...]]) -> Vocabularies:
    builder = VocabularyBuilder()
    for observation, actions in records:
        builder.add(observation, actions)
    return builder.freeze()


def _tensor_fields(value: TensorizedCombatDecision) -> tuple[torch.Tensor, ...]:
    return tuple(
        getattr(value, field.name)
        for field in fields(value)
        if isinstance(getattr(value, field.name), torch.Tensor)
    )


def _assert_different(left: TensorizedCombatDecision, right: TensorizedCombatDecision) -> None:
    assert any(not torch.equal(a, b) for a, b in zip(_tensor_fields(left), _tensor_fields(right)))


def test_combat_action_kinds_match_authoritative_native_schema() -> None:
    native_kinds = tuple(_native.combat_player_choice_kinds())
    assert native_kinds == get_args(PlayerChoiceKind.__value__)
    assert native_kinds == tuple(tensor_module._ACTION_SPECS)


@pytest.mark.parametrize("kind", ("confirm_selection", "skip_selection"))
def test_selection_completion_actions_tensorize_structurally(kind: str) -> None:
    observation, _ = _decision()
    actions = (ActionDescriptor("combat", kind),)
    vocab = _vocab((observation, actions))
    encoded = tensorize_combat(observation, actions, vocab)
    assert encoded.action_count == 1
    assert not encoded.action_source_mask[0]
    assert not encoded.action_target_mask[0]


def test_every_fair_field_has_an_explicit_tensor_disposition() -> None:
    assert field_coverage_mismatches() == {}
    valid_dispositions = {"encoded", "reference_only", "validated", "ignored"}
    assert all(
        disposition in valid_dispositions and bool(reason.strip())
        for fields_by_type in FIELD_COVERAGE.values()
        for disposition, reason in fields_by_type.values()
    )


def test_field_coverage_walks_newly_nested_dataclasses() -> None:
    synthetic_ledger: Mapping[type[object], Mapping[str, tuple[str, str]]] = {
        SyntheticRoot: {"leaf": ("encoded", "synthetic nested field")},
    }
    assert field_coverage_mismatches((SyntheticRoot,), synthetic_ledger) == {
        "SyntheticLeaf": ({"visible_value"}, set())
    }


def test_base_package_does_not_import_or_require_torch() -> None:
    script = """
import builtins
original = builtins.__import__
def guarded(name, *args, **kwargs):
    if name == 'torch' or name.startswith('torch.'):
        raise RuntimeError('base package imported torch')
    return original(name, *args, **kwargs)
builtins.__import__ = guarded
import sts_sim
print(sts_sim.Card.BASH.value)
"""
    completed = subprocess.run(
        [sys.executable, "-c", script], check=False, capture_output=True, text=True
    )
    assert completed.returncode == 0, completed.stderr
    assert completed.stdout.strip() == "Bash"


def test_tensor_module_has_no_privileged_import_or_accessor_seam() -> None:
    path = Path(__file__).parents[1] / "sts_sim" / "rl" / "tensor.py"
    source = path.read_text()
    tree = ast.parse(source)
    forbidden = {"_native", "RunEnv", "full_state", "snapshot", "_handle"}
    referenced = {node.id for node in ast.walk(tree) if isinstance(node, ast.Name)} | {
        node.attr for node in ast.walk(tree) if isinstance(node, ast.Attribute)
    }
    assert forbidden.isdisjoint(referenced)
    assert all(
        not alias.name.endswith(("._native", ".run"))
        for node in ast.walk(tree)
        if isinstance(node, (ast.Import, ast.ImportFrom))
        for alias in node.names
    )


def test_known_observation_schemas_are_encoded_and_unknown_schema_fails_closed() -> None:
    observation, actions = _decision()
    version_one = replace(observation, schema_version=1, orb_slots=())
    version_two = replace(observation, schema_version=2, orb_slots=())
    vocab = _vocab((version_one, actions), (version_two, actions))
    one = tensorize_combat(version_one, actions, vocab)
    two = tensorize_combat(version_two, actions, vocab)
    schema_column = SCALAR_INDEX["observation_schema_version"]
    assert one.entity_scalar_mask[0, schema_column]
    assert two.entity_scalar_mask[0, schema_column]
    assert one.entity_scalars[0, schema_column] == 1
    assert two.entity_scalars[0, schema_column] == 2
    with pytest.raises(ValueError, match="unsupported fair observation schema"):
        tensorize_combat(replace(observation, schema_version=3), actions, vocab)


def test_action_descriptor_strips_revision_and_native_handle() -> None:
    decision = RunEnv.combat_fixture().decision()
    descriptor = decision.actions[0].descriptor()
    assert set(descriptor.__dataclass_fields__) == {
        "family",
        "kind",
        "hand_slot",
        "potion_slot",
        "option_slot",
        "target_slot",
        "card_slot",
        "node_slot",
        "reward_slot",
        "shop_slot",
        "slot",
    }


def test_visible_card_cost_and_every_dynamic_value_change_tensors() -> None:
    observation, actions = _decision()
    card = observation.hand[0].card
    changed_cards = [
        replace(card, cost=card.cost + 1),
        replace(card, cost_is_modified=not card.cost_is_modified),
        replace(card, cost_resets_next_turn=not card.cost_resets_next_turn),
        replace(card, dynamic=replace(card.dynamic, rampage_damage_bonus=3)),
        replace(card, dynamic=replace(card.dynamic, ritual_dagger_damage_bonus=4)),
        replace(card, dynamic=replace(card.dynamic, windmill_retain_damage=5)),
        replace(card, dynamic=replace(card.dynamic, steam_barrier_block_reduction=2)),
        replace(card, dynamic=replace(card.dynamic, combat_cost_under_turn_override=1)),
    ]
    variants = [
        replace(observation, hand=(replace(observation.hand[0], card=value), *observation.hand[1:]))
        for value in changed_cards
    ]
    vocab = _vocab((observation, actions), *((variant, actions) for variant in variants))
    baseline = tensorize_combat(observation, actions, vocab)
    for variant in variants:
        _assert_different(baseline, tensorize_combat(variant, actions, vocab))


def test_card_canonicalization_handles_none_and_integer_dynamic_values() -> None:
    observation, actions = _decision()
    card = observation.hand[0].card
    cards = (
        replace(card, dynamic=replace(card.dynamic, rampage_damage_bonus=None)),
        replace(card, dynamic=replace(card.dynamic, rampage_damage_bonus=3)),
    )
    variant = replace(observation, discard_pile=FairPile(2, cards, ()))
    vocab = _vocab((variant, actions))
    encoded = tensorize_combat(variant, actions, vocab)
    assert encoded.entity_kind.shape[0] > 2


def test_partial_known_order_is_rejected_by_current_observation_contract() -> None:
    observation, actions = _decision()
    cards = tuple(item.card for item in observation.hand[:2])
    partial = replace(
        observation,
        draw_pile=FairPile(count=len(cards), cards=cards, known_order=cards[:1]),
    )
    vocab = _vocab((partial, actions))
    with pytest.raises(ValueError, match="empty or a complete pile permutation"):
        tensorize_combat(partial, actions, vocab)


def test_visible_monster_intent_fields_and_defensive_mode_individually_change_tensors() -> None:
    observation, actions = _decision()
    monster = observation.monsters[0]
    original_intent = monster.intent
    assert original_intent.visibility == "visible"
    variants = [
        replace(monster, in_defensive_mode=not monster.in_defensive_mode),
        replace(monster, intent=replace(original_intent, visibility="hidden")),
        replace(monster, intent=replace(original_intent, category="attack_buff")),
        replace(monster, intent=replace(original_intent, damage=99)),
        replace(monster, intent=replace(original_intent, hits=3)),
    ]
    observations = [replace(observation, monsters=(variant,)) for variant in variants]
    vocab = _vocab((observation, actions), *((variant, actions) for variant in observations))
    baseline = tensorize_combat(observation, actions, vocab)
    for variant in observations:
        _assert_different(baseline, tensorize_combat(variant, actions, vocab))


def test_known_draw_order_relic_counter_and_selection_change_tensors() -> None:
    observation, actions = _decision()
    cards = tuple(item.card for item in observation.hand[:2])
    hidden = replace(
        observation,
        draw_pile=FairPile(count=len(cards), cards=cards, known_order=()),
    )
    known = replace(hidden, draw_pile=replace(hidden.draw_pile, known_order=cards[::-1]))
    relic_slot = len(hidden.relics)
    relic_zero = replace(
        hidden,
        relics=(*hidden.relics, FairRelic(relic_slot, "Ink Bottle", (FairCounter("cards", 0),))),
    )
    relic_eight = replace(
        hidden,
        relics=(*hidden.relics, FairRelic(relic_slot, "Ink Bottle", (FairCounter("cards", 8),))),
    )
    option = FairSelectionOption(0, cards[0])
    selection_clear = replace(
        hidden,
        selection=FairSelection("warcry_put_on_draw", (option,), ()),
    )
    selection_marked = replace(
        hidden,
        selection=FairSelection("warcry_put_on_draw", (option,), (0,)),
    )
    selection_actions = (*actions, ActionDescriptor("combat", "toggle_visible_card", option_slot=0))
    vocab = _vocab(
        (hidden, actions),
        (known, actions),
        (relic_zero, actions),
        (relic_eight, actions),
        (selection_clear, selection_actions),
        (selection_marked, selection_actions),
    )
    baseline = tensorize_combat(hidden, actions, vocab)
    _assert_different(baseline, tensorize_combat(known, actions, vocab))
    _assert_different(
        tensorize_combat(relic_zero, actions, vocab),
        tensorize_combat(relic_eight, actions, vocab),
    )
    _assert_different(
        tensorize_combat(selection_clear, selection_actions, vocab),
        tensorize_combat(selection_marked, selection_actions, vocab),
    )


def test_public_relic_and_orb_order_is_canonical_and_encoded() -> None:
    observation, actions = _decision()
    relics = (
        FairRelic(0, "Burning Blood", ()),
        FairRelic(1, "Ink Bottle", ()),
    )
    orbs = (
        FairOrbSlot(0, FairOrb("lightning", None)),
        FairOrbSlot(1, FairOrb("dark", 12)),
    )
    ordered = replace(observation, relics=relics, orb_slots=orbs)
    reversed_storage = replace(ordered, relics=relics[::-1], orb_slots=orbs[::-1])
    vocab = _vocab((ordered, actions), (reversed_storage, actions))
    left = tensorize_combat(ordered, actions, vocab)
    right = tensorize_combat(reversed_storage, actions, vocab)
    assert all(torch.equal(a, b) for a, b in zip(_tensor_fields(left), _tensor_fields(right)))

    rank_column = SCALAR_INDEX["visible_rank"]
    kind_tokens = vocab.namespaces["entity_kind"].tokens
    relic_kind = kind_tokens.index("relic")
    orb_kind = kind_tokens.index("orb")
    relic_ranks = left.entity_scalars[left.entity_kind == relic_kind, rank_column].tolist()
    orb_ranks = left.entity_scalars[left.entity_kind == orb_kind, rank_column].tolist()
    assert relic_ranks == [0.0, 1.0]
    assert orb_ranks == [0.0, 1.0]

    swapped_public_order = replace(
        ordered,
        relics=(FairRelic(0, "Ink Bottle", ()), FairRelic(1, "Burning Blood", ())),
        orb_slots=(
            FairOrbSlot(0, FairOrb("dark", 12)),
            FairOrbSlot(1, FairOrb("lightning", None)),
        ),
    )
    _assert_different(left, tensorize_combat(swapped_public_order, actions, vocab))


def test_unordered_pile_permutations_are_tensor_invariant() -> None:
    observation, actions = _decision()
    cards = tuple(item.card for item in observation.hand)
    left = replace(observation, discard_pile=FairPile(len(cards), cards, ()))
    right = replace(left, discard_pile=FairPile(len(cards), cards[::-1], ()))
    vocab = _vocab((left, actions), (right, actions))
    left_tensors = tensorize_combat(left, actions, vocab)
    right_tensors = tensorize_combat(right, actions, vocab)
    assert all(
        torch.equal(a, b)
        for a, b in zip(_tensor_fields(left_tensors), _tensor_fields(right_tensors))
    )


def test_hand_permutation_and_descriptor_remap_are_equivariant() -> None:
    observation, actions = _decision()
    count = len(observation.hand)
    slot_map = {old: count - 1 - old for old in range(count)}
    hand = tuple(replace(item, slot=slot_map[item.slot]) for item in reversed(observation.hand))
    remapped = tuple(
        replace(
            action,
            hand_slot=None if action.hand_slot is None else slot_map[action.hand_slot],
        )
        for action in actions
    )
    variant = replace(observation, hand=hand)
    vocab = _vocab((observation, actions), (variant, remapped))
    left = tensorize_combat(observation, actions, vocab)
    right = tensorize_combat(variant, remapped, vocab)
    assert all(torch.equal(a, b) for a, b in zip(_tensor_fields(left), _tensor_fields(right)))


def test_action_rows_preserve_input_order_and_reference_public_entities() -> None:
    observation, actions = _decision()
    vocab = _vocab((observation, actions))
    encoded = tensorize_combat(observation, actions, vocab)
    assert encoded.action_count == len(actions)
    action_vocab = vocab.namespaces["action_kind"].tokens
    family_vocab = vocab.namespaces["action_family"].tokens
    reconstructed_rows = tuple(
        (
            family_vocab[int(encoded.action_family[row])],
            action_vocab[int(encoded.action_kind[row])],
            bool(encoded.action_source_mask[row]),
            bool(encoded.action_target_mask[row]),
        )
        for row in range(encoded.action_count)
    )
    assert reconstructed_rows == tuple(
        (
            action.family,
            action.kind,
            any(
                source is not None
                for source in (action.hand_slot, action.potion_slot, action.option_slot)
            ),
            action.target_slot is not None,
        )
        for action in actions
    )
    for row, action in enumerate(actions):
        if action.hand_slot is not None:
            source = int(encoded.action_source[row])
            card_vocab = vocab.namespaces["card"].tokens
            assert (
                card_vocab[int(encoded.entity_content[source])]
                == observation.hand[action.hand_slot].card.content_key
            )
        if action.target_slot is not None:
            target = int(encoded.action_target[row])
            monster_vocab = vocab.namespaces["monster"].tokens
            assert (
                monster_vocab[int(encoded.entity_content[target])]
                == observation.monsters[action.target_slot].content_key
            )


def test_every_fixture_action_index_round_trips_through_authoritative_sidecar() -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()
    for index in range(len(decision.actions)):
        clone = env.clone()
        result = clone.step(decision.actions[index])
        assert result.decision.revision == 1


def test_vocabulary_is_immutable_and_strictly_round_trips() -> None:
    observation, actions = _decision()
    vocab = _vocab((observation, actions))
    payload = vocab.to_dict()
    restored = Vocabularies.from_dict(payload)
    assert restored.to_dict() == payload
    canonical = json.dumps(
        payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")
    assert vocab.fingerprint == restored.fingerprint == hashlib.sha256(canonical).hexdigest()
    cached = FrozenVocabulary.build(("beta", "alpha"))
    assert cached.encode("alpha") == (3, False)
    assert cached.encode("future") == (1, True)
    assert cached == FrozenVocabulary(tuple(cached.tokens))
    assert "_token_to_index" not in repr(cached)
    with pytest.raises(TypeError):
        cast(dict[str, FrozenVocabulary], vocab.namespaces)["new"] = FrozenVocabulary.build(())
    with pytest.raises(ValueError, match="sorted unique"):
        FrozenVocabulary(("<pad>", "<unk>", "<none>", "z", "a"))
    with pytest.raises(ValueError, match="sorted unique"):
        FrozenVocabulary(("<pad>", "<unk>", "<none>", "a", "a"))
    with pytest.raises(ValueError, match="all be strings"):
        FrozenVocabulary(cast(tuple[str, ...], ("<pad>", "<unk>", "<none>", 1)))
    with pytest.raises(ValueError, match="exact required"):
        Vocabularies.from_dict({key: value for key, value in payload.items() if key != "power"})
    with pytest.raises(ValueError, match="exact required"):
        Vocabularies.from_dict({**payload, "extra": ["<pad>", "<unk>", "<none>"]})
    unsorted = {**payload, "power": ["<pad>", "<unk>", "<none>", "z", "a"]}
    with pytest.raises(ValueError, match="sorted unique"):
        Vocabularies.from_dict(unsorted)
    non_string = {**payload, "power": ["<pad>", "<unk>", "<none>", 1]}
    with pytest.raises(ValueError, match="all be strings"):
        Vocabularies.from_dict(cast(Mapping[str, list[str]], non_string))


def test_multiple_unknown_power_and_counter_keys_are_aggregated_without_loss() -> None:
    observation, actions = _decision()
    vocab = _vocab((observation, actions))
    player = replace(
        observation.player,
        powers=(*observation.player.powers, FairPower("future_one", 2), FairPower("future_two", 3)),
    )
    unknown = replace(
        observation,
        player=player,
        public_counters=(
            *observation.public_counters,
            FairCounter("future_counter_one", 4),
            FairCounter("future_counter_two", 7),
        ),
    )
    encoded = tensorize_combat(unknown, actions, vocab)
    unknown_power = vocab.namespaces["power"].tokens.index("<unk>")
    unknown_counter = vocab.namespaces["counter"].tokens.index("<unk>")
    assert encoded.entity_powers[1, unknown_power] == 5
    assert encoded.entity_power_counts[1, unknown_power] == 2
    assert encoded.entity_power_mask[1, unknown_power]
    assert encoded.entity_counters[0, unknown_counter] == 11
    assert encoded.entity_counter_counts[0, unknown_counter] == 2
    assert encoded.entity_counter_mask[0, unknown_counter]
    assert encoded.oov_counts == {"power": 2, "counter": 2}


def test_vocabulary_unk_accounting_and_hash_seed_determinism() -> None:
    observation, actions = _decision()
    vocab = _vocab((observation, actions))

    unknown = replace(
        observation,
        monsters=(replace(observation.monsters[0], content_key="future visible monster"),),
    )
    encoded = tensorize_combat(unknown, actions, vocab)
    assert encoded.oov_counts == {"monster": 1}

    script = """
import json
from sts_sim import RunEnv
from sts_sim.rl import VocabularyBuilder
D=RunEnv.combat_fixture().decision()
B=VocabularyBuilder(); B.add(D.observation, tuple(a.descriptor() for a in D.actions))
print(json.dumps(B.freeze().to_dict(), sort_keys=True))
"""
    outputs = []
    for seed in ("1", "987654"):
        environment = dict(os.environ, PYTHONHASHSEED=seed)
        completed = subprocess.run(
            [sys.executable, "-c", script],
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )
        assert completed.returncode == 0, completed.stderr
        outputs.append(json.loads(completed.stdout))
    assert outputs[0] == outputs[1] == vocab.to_dict()


def test_dynamic_batch_padding_uses_entity_and_action_masks_without_fixed_limits() -> None:
    observation, actions = _decision()
    short_actions = actions[:1]
    full_observation = replace(
        observation,
        relics=(*observation.relics, FairRelic(len(observation.relics), "Ink Bottle", ())),
    )
    vocab = _vocab((full_observation, actions), (observation, short_actions))
    full = tensorize_combat(full_observation, actions, vocab)
    short = tensorize_combat(observation, short_actions, vocab)
    batch = collate_combat_tensors((full, short))
    assert batch.vocabulary_fingerprint == vocab.fingerprint
    assert batch.action_mask.shape == (2, len(actions))
    assert batch.action_mask[0].all()
    assert batch.action_mask[1, 0]
    assert not batch.action_mask[1, 1:].any()
    assert batch.entity_mask[0].all()
    assert batch.entity_mask[1, :-1].all()
    assert not batch.entity_mask[1, -1]
    assert not batch.entity_scalar_mask[1, -1].any()
    assert not batch.entity_power_mask[1, -1].any()
    assert not batch.entity_counter_mask[1, -1].any()
    assert batch.entity_parent[1, -1] == -1


def test_collation_rejects_same_shape_tensors_from_different_vocabularies() -> None:
    observation, actions = _decision()
    vocab = _vocab((observation, actions))
    payload = vocab.to_dict()
    card_tokens = payload["card"]
    content_tokens = set(card_tokens[3:])
    removed = max(content_tokens)
    content_tokens.remove(removed)
    content_tokens.add(f"{removed}\uffff")
    changed_payload = {
        **payload,
        "card": [*card_tokens[:3], *sorted(content_tokens)],
    }
    changed_vocab = Vocabularies.from_dict(changed_payload)

    original = tensorize_combat(observation, actions, vocab)
    changed = tensorize_combat(observation, actions, changed_vocab)
    assert original.vocabulary_fingerprint != changed.vocabulary_fingerprint
    assert all(
        left.shape == right.shape
        for left, right in zip(_tensor_fields(original), _tensor_fields(changed))
    )
    with pytest.raises(ValueError, match="different vocabularies"):
        collate_combat_tensors((original, changed))


@pytest.mark.parametrize(
    "descriptor",
    (
        ActionDescriptor("map", "choose_map_node", node_slot=1),
        ActionDescriptor("potion", "use_potion_slot", potion_slot=0),
        ActionDescriptor("combat", "unknown_kind"),
        ActionDescriptor("combat", "play_hand_slot"),
        ActionDescriptor("combat", "play_hand_slot", hand_slot=0, potion_slot=0),
        ActionDescriptor("combat", "end_turn", hand_slot=0),
        ActionDescriptor("combat", "end_turn", target_slot=0),
        ActionDescriptor("combat", "use_potion_slot"),
        ActionDescriptor("combat", "confirm_selection", option_slot=0),
        ActionDescriptor("combat", "discard_potion_slot", potion_slot=0, target_slot=0),
        ActionDescriptor("combat", "end_turn", card_slot=0),
    ),
)
def test_malformed_combat_action_descriptors_fail_closed(descriptor: ActionDescriptor) -> None:
    observation, actions = _decision()
    bad = (descriptor,)
    vocab = _vocab((observation, actions), (observation, bad))
    with pytest.raises(ValueError):
        tensorize_combat(observation, bad, vocab)


def test_noncontiguous_public_slots_fail_closed() -> None:
    observation, actions = _decision()
    card = observation.hand[0].card
    variants = (
        replace(
            observation,
            hand=(replace(observation.hand[0], slot=9), *observation.hand[1:]),
        ),
        replace(
            observation,
            relics=(*observation.relics, FairRelic(len(observation.relics) + 1, "Ink Bottle", ())),
        ),
        replace(observation, orb_slots=(FairOrbSlot(1, FairOrb("lightning", None)),)),
        replace(
            observation,
            selection=FairSelection("warcry_put_on_draw", (FairSelectionOption(1, card),), ()),
        ),
    )
    vocab = _vocab((observation, actions), *((variant, actions) for variant in variants))
    for variant in variants:
        with pytest.raises(ValueError, match="slots must be unique and contiguous"):
            tensorize_combat(variant, actions, vocab)
