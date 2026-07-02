use sts_core::{
    apply_combat_action,
    card::{CardType, TargetRequirement},
    content::{
        cards,
        monsters::{monster_state, FIXED_SIMPLE_MONSTER},
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState, MonsterId,
};

#[test]
fn wound_definition_matches_unplayable_status_source() {
    assert_eq!(cards::WOUND.cost, 0);
    assert_eq!(cards::WOUND.card_type, CardType::Status);
    assert_eq!(cards::WOUND.target, TargetRequirement::None);
    assert_eq!(cards::WOUND.values.damage, None);
    assert!(cards::WOUND.keywords.unplayable);
    assert!(!cards::WOUND.keywords.exhaust);
}

#[test]
fn pummel_plus_definition_keeps_damage_and_exhausts() {
    assert_eq!(cards::PUMMEL.values.damage, Some(2));
    assert!(cards::PUMMEL.keywords.exhaust);

    assert_eq!(cards::PUMMEL_PLUS.values.damage, Some(2));
    assert!(cards::PUMMEL_PLUS.keywords.exhaust);
}

#[test]
fn twin_strike_definitions_target_one_enemy_twice() {
    assert_eq!(cards::TWIN_STRIKE.target, TargetRequirement::Enemy);
    assert_eq!(cards::TWIN_STRIKE.values.damage, Some(5));

    assert_eq!(cards::TWIN_STRIKE_PLUS.target, TargetRequirement::Enemy);
    assert_eq!(cards::TWIN_STRIKE_PLUS.values.damage, Some(7));
}

#[test]
fn havoc_flash_of_steel_plus_deals_damage_draws_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::FLASH_OF_STEEL_PLUS_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Havoc plays Flash of Steel+ from the draw pile");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
    assert!(next
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == cards::STRIKE_R_ID));
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::FLASH_OF_STEEL_PLUS_ID
    );
}

#[test]
fn havoc_master_of_strategy_plus_draws_four_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::SHRUG_IT_OFF_ID),
        CardInstance::new(CardId::new(6), cards::MASTER_OF_STRATEGY_PLUS_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Havoc plays Master of Strategy+ from the draw pile");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.hand.len(), 4);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::MASTER_OF_STRATEGY_PLUS_ID));
}

#[test]
fn havoc_battle_trance_plus_draws_four_sets_no_draw_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::SHRUG_IT_OFF_ID),
        CardInstance::new(CardId::new(6), cards::BATTLE_TRANCE_PLUS_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Havoc plays Battle Trance+ from the draw pile");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.hand.len(), 4);
    assert!(next.player.cannot_draw);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::BATTLE_TRANCE_PLUS_ID));
}

#[test]
fn deep_breath_shuffles_discard_before_drawing() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DEEP_BREATH_ID)];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), cards::DEFEND_R_ID)];
    state.piles.discard_pile = vec![
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Deep Breath plays without a monster target");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::DEEP_BREATH_ID);
    assert_eq!(next.piles.draw_pile.len(), 2);
}

#[test]
fn deep_breath_plus_draws_two_after_shuffle() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::DEEP_BREATH_PLUS_ID,
    )];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), cards::DEFEND_R_ID)];
    state.piles.discard_pile = vec![
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Deep Breath+ plays without a monster target");

    assert_eq!(next.piles.hand.len(), 2);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::DEEP_BREATH_PLUS_ID
    );
    assert_eq!(next.piles.draw_pile.len(), 1);
}

#[test]
fn impatience_plays_with_attack_in_hand_without_drawing() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::IMPATIENCE_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
    ];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    assert!(
        legal_combat_actions(&state).contains(&CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        })
    );

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Impatience remains playable with an attack in hand");

    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::IMPATIENCE_ID);
}

#[test]
fn impatience_plus_draws_three_without_attack_in_hand() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::IMPATIENCE_PLUS_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::SHRUG_IT_OFF_ID),
        CardInstance::new(CardId::new(4), cards::BATTLE_TRANCE_ID),
    ];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Impatience+ draws when no attacks are in hand");

    assert_eq!(next.piles.hand.len(), 3);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::IMPATIENCE_PLUS_ID
    );
}

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
