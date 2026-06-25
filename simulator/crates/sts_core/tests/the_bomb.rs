use sts_core::{
    apply_combat_action,
    card::CardType,
    content::cards,
    content::monsters::{monster_state, FIXED_SIMPLE_MONSTER},
    CardId, CardInstance, CombatAction, CombatState, MonsterId, TargetRequirement,
};

fn hand_card_id(state: &CombatState, content_id: sts_core::ContentId) -> CardId {
    state
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == content_id)
        .expect("card is in hand")
        .id
}

fn bomb_fixture() -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 80;
    state.player.energy = 3;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::THE_BOMB_ID)];
    state.piles.draw_pile.clear();
    state.piles.discard_pile.clear();
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    for monster in &mut state.monsters {
        monster.hp = 50;
    }
    state
}

#[test]
fn the_bomb_has_base_colorless_rare_skill_values() {
    assert_eq!(cards::THE_BOMB.cost, 2);
    assert_eq!(cards::THE_BOMB.card_type, CardType::Skill);
    assert_eq!(cards::THE_BOMB.target, TargetRequirement::None);
    assert_eq!(cards::THE_BOMB.values.damage, Some(cards::THE_BOMB_DAMAGE));
    assert_eq!(cards::THE_BOMB_TURNS, 3);
    assert_eq!(
        cards::card_type_and_rarity(cards::THE_BOMB_ID),
        Some((CardType::Skill, sts_core::card::CardRarity::Rare))
    );
}

#[test]
fn playing_the_bomb_arms_timer_and_discards_card() {
    let state = bomb_fixture();
    let bomb_id = hand_card_id(&state, cards::THE_BOMB_ID);

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: bomb_id,
            target: None,
        },
    )
    .expect("The Bomb plays without target");

    assert_eq!(next.player.energy, 1);
    assert_eq!(next.bomb_timers.len(), 1);
    assert_eq!(next.bomb_timers[0].turns_remaining, cards::THE_BOMB_TURNS);
    assert_eq!(next.bomb_timers[0].damage, cards::THE_BOMB_DAMAGE);
    assert!(next
        .piles
        .discard_pile
        .iter()
        .any(|card| card.id == bomb_id));
}

#[test]
fn the_bomb_explodes_after_three_end_turn_ticks() {
    let state = bomb_fixture();
    let bomb_id = hand_card_id(&state, cards::THE_BOMB_ID);
    let mut state = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: bomb_id,
            target: None,
        },
    )
    .expect("The Bomb plays");

    state = sts_core::end_player_turn(&state);
    assert_eq!(state.bomb_timers[0].turns_remaining, 2);
    assert!(state.monsters.iter().all(|monster| monster.hp == 50));

    state = sts_core::end_player_turn(&state);
    assert_eq!(state.bomb_timers[0].turns_remaining, 1);
    assert!(state.monsters.iter().all(|monster| monster.hp == 50));

    state = sts_core::end_player_turn(&state);
    assert!(state.bomb_timers.is_empty());
    assert!(state.monsters.iter().all(|monster| monster.hp == 10));
}
