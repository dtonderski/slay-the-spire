from dataclasses import replace

from sts_sim import FairCombatObservation, FairOrb, FairOrbSlot, RunEnv
from sts_sim.notebook import action_label, format_action, format_actions, render_decision


def test_render_decision_covers_state_and_legal_actions() -> None:
    decision = RunEnv.combat_fixture().decision()
    assert isinstance(decision.observation, FairCombatObservation)

    rendered = render_decision(decision)

    assert "Combat |" in rendered
    assert "Hand (" in rendered
    assert "Piles" in rendered
    assert "Monsters" in rendered
    assert "Relics" in rendered
    assert "Potions" in rendered
    assert "Legal actions" in rendered
    assert all(action_label(decision, action) in rendered for action in decision.actions)


def test_action_label_resolves_public_slots_not_tuple_positions() -> None:
    decision = RunEnv.combat_fixture().decision()
    assert isinstance(decision.observation, FairCombatObservation)
    play = next(action for action in decision.actions if action.kind == "play_hand_slot")
    assert play.hand_slot is not None
    expected = next(
        visible.card.content_key
        for visible in decision.observation.hand
        if visible.slot == play.hand_slot
    )
    reordered = replace(
        decision,
        observation=replace(
            decision.observation,
            hand=tuple(reversed(decision.observation.hand)),
        ),
    )

    assert expected.replace("_", " ") in action_label(reordered, play)


def test_render_decision_keeps_public_content_as_text() -> None:
    decision = RunEnv.combat_fixture().decision()
    assert isinstance(decision.observation, FairCombatObservation)
    first = decision.observation.hand[0]
    unusual = replace(first, card=replace(first.card, content_key="card < demo"))
    changed = replace(
        decision,
        observation=replace(
            decision.observation,
            hand=(unusual, *decision.observation.hand[1:]),
        ),
    )

    rendered = render_decision(changed)

    assert "Card < Demo" in rendered


def test_combat_formatter_includes_orbs_and_windmill_damage() -> None:
    decision = RunEnv.combat_fixture().decision()
    assert isinstance(decision.observation, FairCombatObservation)
    first = decision.observation.hand[0]
    windmill = replace(
        first,
        card=replace(
            first.card,
            dynamic=replace(first.card.dynamic, windmill_retain_damage=8),
        ),
    )
    changed = replace(
        decision,
        observation=replace(
            decision.observation,
            orb_slots=(
                FairOrbSlot(0, FairOrb("lightning", None)),
                FairOrbSlot(1, FairOrb("dark", 24)),
                FairOrbSlot(2, None),
            ),
            hand=(windmill, *decision.observation.hand[1:]),
        ),
    )

    rendered = render_decision(changed)

    assert "Orbs: [0] Lightning, [1] Dark (evoke 24), [2] empty" in rendered
    assert "Windmill +8" in rendered


def test_noncombat_decision_still_renders_legal_actions() -> None:
    decision = RunEnv.new_ironclad("ABC123").decision()

    rendered = render_decision(decision)

    assert "Event: Neow" in rendered
    assert "Choices:" in rendered
    assert "Deck (" in rendered
    assert "Legal actions" in rendered
    assert "Choose Talk" in rendered


def test_printing_an_observation_uses_the_plain_text_formatter() -> None:
    observation = RunEnv.new_ironclad("ABC123").observation()

    rendered = str(observation)

    assert "Event: Neow" in rendered
    assert "FairRunObservation(" not in rendered


def test_action_formatters_include_labels_and_public_slots() -> None:
    decision = RunEnv.combat_fixture().decision()
    action = decision.actions[0]

    one = format_action(decision, action, index=0)
    many = format_actions(decision)

    assert one.startswith("[0] Play ")
    assert "hand=0" in one
    assert "target=0" in one
    assert one in many
