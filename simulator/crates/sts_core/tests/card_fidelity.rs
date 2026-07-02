use sts_core::{
    apply_combat_action,
    card::TargetRequirement,
    content::{
        cards,
        monsters::{monster_state, FIXED_SIMPLE_MONSTER},
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState, MonsterId,
};

#[test]
fn sword_boomerang_definitions_target_all_enemies_without_selection() {
    for definition in [cards::SWORD_BOOMERANG, cards::SWORD_BOOMERANG_PLUS] {
        assert_eq!(definition.target, TargetRequirement::AllEnemies);
        assert_eq!(definition.values.damage, Some(3));
    }

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SWORD_BOOMERANG_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    assert!(
        legal_combat_actions(&state).contains(&CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        })
    );

    let err = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect_err("Sword Boomerang should not accept a selected target");

    assert_eq!(
        err,
        sts_core::SimError::IllegalAction("all-enemies card cannot have a target")
    );
}

#[test]
fn slimed_plays_without_monster_target_and_exhausts_itself() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SLIMED_ID)];
    state.piles.draw_pile.clear();
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    assert_eq!(cards::SLIMED.target, TargetRequirement::None);
    assert_eq!(cards::SLIMED.values.damage, None);

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Slimed plays without a monster target");

    assert_eq!(next.player.energy, 0);
    assert!(next.piles.hand.is_empty());
    assert!(next.piles.discard_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::SLIMED_ID);
    assert_eq!(next.monsters[0].hp, state.monsters[0].hp);
}

#[test]
fn slimed_rejects_monster_target() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SLIMED_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let err = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect_err("Slimed should not accept a monster target");

    assert_eq!(
        err,
        sts_core::SimError::IllegalAction("non-targeted card cannot have a target")
    );
}
