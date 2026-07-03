use sts_core::{
    apply_combat_action, apply_combat_action_on_run,
    card::{CardType, TargetRequirement},
    combat::transition::{choose_hand_select, confirm_hand_select},
    content::{
        cards,
        monsters::{monster_state, FIXED_SIMPLE_MONSTER},
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState, MonsterId, RunPhase,
    RunState,
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
fn trip_plus_targets_all_enemies_without_selection() {
    assert_eq!(cards::TRIP.target, TargetRequirement::Enemy);
    assert_eq!(cards::TRIP.values.vulnerable, Some(2));

    assert_eq!(cards::TRIP_PLUS.target, TargetRequirement::AllEnemies);
    assert_eq!(cards::TRIP_PLUS.values.vulnerable, Some(2));

    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::TRIP_PLUS_ID)];
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
    .expect_err("Trip+ should not accept a selected target");

    assert_eq!(
        err,
        sts_core::SimError::IllegalAction("all-enemies card cannot have a target")
    );
}

#[test]
fn bandage_up_heals_four_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.max_hp = 60;
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BANDAGE_UP_ID)];
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
    .expect("Bandage Up plays without a target");

    assert_eq!(next.player.hp, 54);
    assert!(next.piles.discard_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::BANDAGE_UP_ID);
}

#[test]
fn bandage_up_plus_heals_six_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.max_hp = 60;
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BANDAGE_UP_PLUS_ID)];
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
    .expect("Bandage Up+ plays without a target");

    assert_eq!(next.player.hp, 56);
    assert!(next.piles.discard_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::BANDAGE_UP_PLUS_ID
    );
}

#[test]
fn bite_heals_two_even_when_damage_is_blocked() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.max_hp = 60;
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BITE_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].block = 99;
    let starting_monster_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Bite plays against a blocked target");

    assert_eq!(next.player.hp, 52);
    assert_eq!(next.monsters[0].hp, starting_monster_hp);
    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::BITE_ID);
}

#[test]
fn whirlwind_definitions_are_x_cost_all_enemy_attacks() {
    assert_eq!(cards::WHIRLWIND.cost, -1);
    assert_eq!(cards::WHIRLWIND.target, TargetRequirement::AllEnemies);
    assert_eq!(cards::WHIRLWIND.values.damage, Some(5));

    assert_eq!(cards::WHIRLWIND_PLUS.cost, -1);
    assert_eq!(cards::WHIRLWIND_PLUS.target, TargetRequirement::AllEnemies);
    assert_eq!(cards::WHIRLWIND_PLUS.values.damage, Some(8));
}

#[test]
fn transmutation_definitions_are_x_cost_exhausting_skills() {
    assert_eq!(cards::TRANSMUTATION.cost, -1);
    assert_eq!(cards::TRANSMUTATION.card_type, CardType::Skill);
    assert_eq!(cards::TRANSMUTATION.target, TargetRequirement::None);
    assert!(cards::TRANSMUTATION.keywords.exhaust);

    assert_eq!(cards::TRANSMUTATION_PLUS.cost, -1);
    assert_eq!(cards::TRANSMUTATION_PLUS.card_type, CardType::Skill);
    assert_eq!(cards::TRANSMUTATION_PLUS.target, TargetRequirement::None);
    assert!(cards::TRANSMUTATION_PLUS.keywords.exhaust);
}

#[test]
fn discovery_plus_keeps_cost_one_and_removes_exhaust() {
    assert_eq!(cards::DISCOVERY.cost, 1);
    assert!(cards::DISCOVERY.keywords.exhaust);

    assert_eq!(cards::DISCOVERY_PLUS.cost, 1);
    assert!(!cards::DISCOVERY_PLUS.keywords.exhaust);
}

#[test]
fn discovery_plus_spends_one_energy_and_delays_non_exhausting_source() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DISCOVERY_PLUS_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Discovery+ plays without a target");

    assert_eq!(next.player.energy, 0);
    assert!(next.piles.hand.is_empty());
    assert!(next.piles.exhaust_pile.is_empty());
    assert!(next.discovery_card_reward.is_some());
    assert_eq!(
        next.discovery_source_card
            .as_ref()
            .map(|card| card.content_id),
        Some(cards::DISCOVERY_PLUS_ID)
    );

    sts_core::combat::transition::close_discovery_card_reward_source(&mut next)
        .expect("closing Discovery reward moves the source card");

    assert!(next.piles.exhaust_pile.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::DISCOVERY_PLUS_ID
    );
}

#[test]
fn forethought_places_selected_card_on_bottom_of_draw_pile() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FORETHOUGHT_ID),
        CardInstance::new(CardId::new(2), cards::BASH_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), cards::STRIKE_R_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Forethought opens hand selection");

    choose_hand_select(&mut next, 0).expect("select the only non-source hand card");
    confirm_hand_select(&mut next).expect("confirm Forethought hand selection");

    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::BASH_ID);
    assert_eq!(next.piles.draw_pile[0].temp_cost, Some(0));
    assert_eq!(next.piles.draw_pile[1].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::FORETHOUGHT_ID);
}

#[test]
fn hand_of_greed_gains_gold_when_it_kills_non_minion() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAND_OF_GREED_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 20;
    state.monsters[0].max_hp = 20;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Hand of Greed kills the target");

    assert_eq!(next.monsters[0].hp, 0);
    assert_eq!(next.combat_gold_gained, 20);
}

#[test]
fn hand_of_greed_does_not_gain_gold_when_it_kills_minion() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAND_OF_GREED_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 20;
    state.monsters[0].max_hp = 20;
    state.monsters[0].powers.minion = 1;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Hand of Greed kills the minion");

    assert_eq!(next.monsters[0].hp, 0);
    assert_eq!(next.combat_gold_gained, 0);
}

#[test]
fn hand_of_greed_gold_transfers_to_run_gold() {
    let mut run = RunState::combat_fixture();
    run.gold = 99;
    let combat = run.combat.as_mut().expect("combat fixture");
    combat.player.energy = 2;
    combat.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::HAND_OF_GREED_PLUS_ID,
    )];
    combat.piles.discard_pile.clear();
    combat.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    combat.monsters[0].hp = 25;
    combat.monsters[0].max_hp = 25;

    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Hand of Greed+ transfers gold to run");

    assert_eq!(next.gold, 124);
    assert_eq!(next.phase, RunPhase::Reward);
    assert!(next.reward.is_some());
    assert!(next.combat.is_none());
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
fn reckless_charge_adds_generated_dazed_to_draw_pile() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::RECKLESS_CHARGE_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Reckless Charge plays and shuffles Dazed into the draw pile");

    assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 7);
    assert_eq!(next.piles.draw_pile.len(), 3);
    assert!(next
        .piles
        .draw_pile
        .iter()
        .any(|card| { card.content_id == cards::DAZED_ID && card.combat_only }));
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::RECKLESS_CHARGE_ID
    );
}

#[test]
fn havoc_reckless_charge_plus_adds_generated_dazed_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::RECKLESS_CHARGE_PLUS_ID),
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
    .expect("Havoc plays Reckless Charge+ from the draw pile");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 10);
    assert_eq!(next.piles.draw_pile.len(), 2);
    assert!(next
        .piles
        .draw_pile
        .iter()
        .any(|card| { card.content_id == cards::DAZED_ID && card.combat_only }));
    assert!(next
        .piles
        .draw_pile
        .iter()
        .any(|card| card.content_id == cards::STRIKE_R_ID));
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::RECKLESS_CHARGE_PLUS_ID
    );
}

#[test]
fn chrysalis_adds_three_zero_cost_generated_skills_to_draw_pile() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::CHRYSALIS_ID)];
    state.piles.draw_pile.clear();
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
    .expect("Chrysalis plays without a target");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.draw_pile.len(), 3);
    assert!(next.piles.draw_pile.iter().all(|card| {
        card.combat_only && card.temp_cost == Some(0) && !card.temp_cost_turn_only
    }));
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::CHRYSALIS_ID);
}

#[test]
fn havoc_chrysalis_plus_adds_five_zero_cost_generated_skills_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::CHRYSALIS_PLUS_ID),
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
    .expect("Havoc plays Chrysalis+ from the draw pile");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.draw_pile.len(), 6);
    assert!(next
        .piles
        .draw_pile
        .iter()
        .any(|card| card.content_id == cards::STRIKE_R_ID));
    assert_eq!(
        next.piles
            .draw_pile
            .iter()
            .filter(|card| {
                card.combat_only && card.temp_cost == Some(0) && !card.temp_cost_turn_only
            })
            .count(),
        5
    );
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::CHRYSALIS_PLUS_ID
    );
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
