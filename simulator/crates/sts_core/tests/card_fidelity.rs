use sts_core::{
    apply_combat_action, apply_combat_action_on_run,
    card::{CardType, TargetRequirement},
    combat::{
        transition::{
            apply_play_top_draw_card_action, choose_draw_select, choose_exhaust_select,
            choose_hand_select, confirm_draw_select, confirm_exhaust_select, confirm_hand_select,
        },
        turn::start_player_turn,
    },
    content::{
        cards,
        monsters::{monster_state, FIXED_SIMPLE_MONSTER},
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState, MonsterId, RunPhase,
    RunState, StsRng,
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
fn carnage_plus_keeps_ethereal_when_upgraded() {
    assert_eq!(cards::CARNAGE.values.damage, Some(20));
    assert!(cards::CARNAGE.keywords.ethereal);

    assert_eq!(cards::CARNAGE_PLUS.values.damage, Some(28));
    assert!(cards::CARNAGE_PLUS.keywords.ethereal);
}

#[test]
fn disarm_plus_keeps_exhaust_and_reduces_three_strength() {
    assert!(cards::DISARM.keywords.exhaust);
    assert!(cards::DISARM_PLUS.keywords.exhaust);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DISARM_PLUS_ID)];
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
    .expect("Disarm+ plays against an enemy");

    assert_eq!(next.monsters[0].powers.strength, -3);
    assert!(next.piles.discard_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::DISARM_PLUS_ID);
}

#[test]
fn ghostly_armor_plus_keeps_ethereal_and_gains_thirteen_block() {
    assert_eq!(cards::GHOSTLY_ARMOR.values.block, Some(10));
    assert!(cards::GHOSTLY_ARMOR.keywords.ethereal);

    assert_eq!(cards::GHOSTLY_ARMOR_PLUS.values.block, Some(13));
    assert!(cards::GHOSTLY_ARMOR_PLUS.keywords.ethereal);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::GHOSTLY_ARMOR_PLUS_ID,
    )];
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Ghostly Armor+ plays without a target");

    assert_eq!(next.player.block, 13);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::GHOSTLY_ARMOR_PLUS_ID
    );
}

#[test]
fn impervious_plus_keeps_exhaust_and_gains_forty_block() {
    assert_eq!(cards::IMPERVIOUS.values.block, Some(30));
    assert!(cards::IMPERVIOUS.keywords.exhaust);

    assert_eq!(cards::IMPERVIOUS_PLUS.values.block, Some(40));
    assert!(cards::IMPERVIOUS_PLUS.keywords.exhaust);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.player.block = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::IMPERVIOUS_PLUS_ID)];
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Impervious+ plays without a target");

    assert_eq!(next.player.block, 40);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::IMPERVIOUS_PLUS_ID
    );
}

#[test]
fn infernal_blade_plus_keeps_exhaust_when_cost_reduces_to_zero() {
    assert_eq!(cards::INFERNAL_BLADE.cost, 1);
    assert!(cards::INFERNAL_BLADE.keywords.exhaust);

    assert_eq!(cards::INFERNAL_BLADE_PLUS.cost, 0);
    assert!(cards::INFERNAL_BLADE_PLUS.keywords.exhaust);
}

#[test]
fn intimidate_plus_keeps_exhaust_and_applies_two_weak_to_all_enemies() {
    assert!(cards::INTIMIDATE.keywords.exhaust);
    assert!(cards::INTIMIDATE_PLUS.keywords.exhaust);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::INTIMIDATE_PLUS_ID)];
    state.piles.exhaust_pile.clear();
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Intimidate+ plays without a target");

    assert_eq!(next.monsters[0].powers.weak, 2);
    assert_eq!(next.monsters[1].powers.weak, 2);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::INTIMIDATE_PLUS_ID
    );
}

#[test]
fn twin_strike_definitions_target_one_enemy_twice() {
    assert_eq!(cards::TWIN_STRIKE.target, TargetRequirement::Enemy);
    assert_eq!(cards::TWIN_STRIKE.values.damage, Some(5));

    assert_eq!(cards::TWIN_STRIKE_PLUS.target, TargetRequirement::Enemy);
    assert_eq!(cards::TWIN_STRIKE_PLUS.values.damage, Some(7));
}

#[test]
fn demon_form_definitions_have_no_damage_and_grant_ritual() {
    assert_eq!(cards::DEMON_FORM.values.damage, None);
    assert_eq!(cards::DEMON_FORM_PLUS.values.damage, None);

    let mut base = CombatState::initial_fixture();
    base.player.energy = 3;
    base.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DEMON_FORM_ID)];
    let next = apply_combat_action(
        &base,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Demon Form grants Ritual");
    assert_eq!(next.player.powers.ritual, 2);

    let mut upgraded = CombatState::initial_fixture();
    upgraded.player.energy = 3;
    upgraded.piles.hand = vec![CardInstance::new(CardId::new(2), cards::DEMON_FORM_PLUS_ID)];
    let next = apply_combat_action(
        &upgraded,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Demon Form+ grants upgraded Ritual");
    assert_eq!(next.player.powers.ritual, 3);
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
        CardInstance::new(CardId::new(4), cards::DEFEND_R_ID),
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

    choose_hand_select(&mut next, 0).expect("select Bash");
    confirm_hand_select(&mut next).expect("confirm Forethought hand selection");

    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::BASH_ID);
    assert_eq!(next.piles.draw_pile[0].temp_cost, Some(0));
    assert_eq!(next.piles.draw_pile[1].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::FORETHOUGHT_ID);
}

#[test]
fn forethought_plays_with_no_other_cards_and_discards_source() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::FORETHOUGHT_ID)];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), cards::STRIKE_R_ID)];
    state.piles.discard_pile.clear();

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
    .expect("Forethought is playable with no other hand cards");

    assert!(next.hand_select.is_none());
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::FORETHOUGHT_ID);
}

#[test]
fn base_forethought_auto_places_only_other_card_on_bottom_of_draw_pile() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FORETHOUGHT_ID),
        CardInstance::new(CardId::new(2), cards::BASH_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), cards::STRIKE_R_ID)];
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("base Forethought auto-moves the only other hand card");

    assert!(next.hand_select.is_none());
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::BASH_ID);
    assert_eq!(next.piles.draw_pile[0].temp_cost, Some(0));
    assert_eq!(next.piles.draw_pile[1].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::FORETHOUGHT_ID);
}

#[test]
fn forethought_plus_places_multiple_selected_cards_on_bottom_of_draw_pile() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FORETHOUGHT_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::BASH_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::ANGER_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(5), cards::STRIKE_R_ID)];
    state.piles.discard_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Forethought+ opens any-number hand selection");

    choose_hand_select(&mut next, 0).expect("select Bash");
    choose_hand_select(&mut next, 1).expect("select Defend");
    confirm_hand_select(&mut next).expect("confirm Forethought+ hand selection");

    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::ANGER_ID);
    assert_eq!(next.piles.draw_pile.len(), 3);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.draw_pile[0].temp_cost, Some(0));
    assert_eq!(next.piles.draw_pile[1].content_id, cards::BASH_ID);
    assert_eq!(next.piles.draw_pile[1].temp_cost, Some(0));
    assert_eq!(next.piles.draw_pile[2].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::FORETHOUGHT_PLUS_ID
    );
}

#[test]
fn forethought_plus_can_confirm_zero_selected_cards() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FORETHOUGHT_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::BASH_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), cards::STRIKE_R_ID)];
    state.piles.discard_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Forethought+ opens any-number hand selection");

    confirm_hand_select(&mut next).expect("Forethought+ can choose zero cards");

    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::BASH_ID);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::FORETHOUGHT_PLUS_ID
    );
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
fn ritual_dagger_gains_three_damage_on_fatal_non_minion_kill() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 15;
    state.monsters[0].max_hp = 15;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Ritual Dagger kills the target");

    assert_eq!(next.monsters[0].hp, 0);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::RITUAL_DAGGER_ID
    );
    assert_eq!(next.piles.exhaust_pile[0].ritual_dagger_damage_bonus, 3);
}

#[test]
fn ritual_dagger_does_not_grow_on_minion_kill() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID)];
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 15;
    state.monsters[0].max_hp = 15;
    state.monsters[0].powers.minion = 1;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Ritual Dagger kills the minion");

    assert_eq!(next.monsters[0].hp, 0);
    assert_eq!(next.piles.exhaust_pile[0].ritual_dagger_damage_bonus, 0);
}

#[test]
fn upgraded_ritual_dagger_grows_by_five_without_changing_content_id() {
    let upgraded =
        cards::upgrade_card_instance(CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID))
            .expect("Ritual Dagger is upgradeable");
    assert_eq!(upgraded.content_id, cards::RITUAL_DAGGER_ID);
    assert_eq!(upgraded.upgrades, 1);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![upgraded];
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 15;
    state.monsters[0].max_hp = 15;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("upgraded Ritual Dagger kills the target");

    assert_eq!(next.piles.exhaust_pile[0].ritual_dagger_damage_bonus, 5);
    assert_eq!(next.piles.exhaust_pile[0].upgrades, 1);
}

#[test]
fn ritual_dagger_damage_growth_transfers_to_run_deck() {
    let mut run = RunState::combat_fixture();
    run.deck = vec![CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID)];
    let combat = run.combat.as_mut().expect("combat fixture");
    combat.player.energy = 1;
    combat.piles.hand = run.deck.clone();
    combat.piles.draw_pile.clear();
    combat.piles.discard_pile.clear();
    combat.piles.exhaust_pile.clear();
    combat.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    combat.monsters[0].hp = 15;
    combat.monsters[0].max_hp = 15;

    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Ritual Dagger kill updates the run deck");

    assert_eq!(next.deck[0].ritual_dagger_damage_bonus, 3);
}

#[test]
fn top_draw_ritual_dagger_grows_on_fatal_non_minion_kill() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand.clear();
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 15;
    state.monsters[0].max_hp = 15;

    let next = apply_play_top_draw_card_action(&state, Some(MonsterId::new(1)))
        .expect("top-draw Ritual Dagger kills the target");

    assert_eq!(next.monsters[0].hp, 0);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::RITUAL_DAGGER_ID
    );
    assert_eq!(next.piles.exhaust_pile[0].ritual_dagger_damage_bonus, 3);
}

#[test]
fn madness_prefers_cards_with_positive_cost_for_turn() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    let mut strike = CardInstance::new(CardId::new(2), cards::STRIKE_R_ID);
    strike.temp_cost = Some(0);
    strike.temp_cost_turn_only = true;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::MADNESS_ID),
        strike,
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Madness plays");

    let strike = next
        .piles
        .hand
        .iter()
        .find(|card| card.id == CardId::new(2))
        .expect("Strike remains in hand");
    let defend = next
        .piles
        .hand
        .iter()
        .find(|card| card.id == CardId::new(3))
        .expect("Defend remains in hand");
    assert_eq!(strike.temp_cost, Some(0));
    assert!(strike.temp_cost_turn_only);
    assert_eq!(defend.temp_cost, Some(0));
    assert!(!defend.temp_cost_turn_only);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::MADNESS_ID);
}

#[test]
fn mayhem_plus_costs_one_and_grants_mayhem() {
    assert_eq!(cards::MAYHEM.cost, 2);
    assert_eq!(cards::MAYHEM_PLUS.cost, 1);
    assert_eq!(
        cards::upgrade_content_id(cards::MAYHEM_ID),
        Some(cards::MAYHEM_PLUS_ID)
    );

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::MAYHEM_PLUS_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Mayhem+ plays without a target");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.player.powers.mayhem, 1);
    assert!(next.piles.hand.is_empty());
    assert!(next.piles.discard_pile.is_empty());
    assert!(next.piles.exhaust_pile.is_empty());
}

#[test]
fn metamorphosis_keeps_generated_x_cost_attacks_x_cost() {
    let mut matching_state = None;
    for seed in 0..10_000 {
        let mut state = CombatState::initial_fixture();
        state.card_random_rng = Some(StsRng::new(seed));
        state.player.energy = 2;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::METAMORPHOSIS_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Metamorphosis plays without a target");

        if next
            .piles
            .draw_pile
            .iter()
            .any(|card| card.content_id == cards::WHIRLWIND_ID)
        {
            matching_state = Some(next);
            break;
        }
    }

    let next = matching_state.expect("test search should find a Whirlwind roll");
    let whirlwind = next
        .piles
        .draw_pile
        .iter()
        .find(|card| card.content_id == cards::WHIRLWIND_ID)
        .expect("matching state includes generated Whirlwind");
    assert!(whirlwind.combat_only);
    assert_eq!(whirlwind.temp_cost, None);

    assert!(next.piles.draw_pile.iter().any(|card| {
        card.combat_only && card.content_id != cards::WHIRLWIND_ID && card.temp_cost == Some(0)
    }));
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::METAMORPHOSIS_ID
    );
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
fn havoc_mind_blast_plus_uses_remaining_draw_pile_size_only() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::HAVOC_ID),
        CardInstance::new(CardId::new(4), cards::DEFEND_R_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::MIND_BLAST_PLUS_ID),
    ];
    state.piles.discard_pile = vec![
        CardInstance::new(CardId::new(5), cards::BASH_ID),
        CardInstance::new(CardId::new(6), cards::ANGER_ID),
    ];
    state.piles.exhaust_pile.clear();
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
    .expect("Havoc plays Mind Blast+ from the draw pile");

    assert_eq!(next.monsters[0].hp, 19);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 3);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert!(next
        .piles
        .discard_pile
        .iter()
        .any(|card| card.content_id == cards::HAVOC_ID));
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::MIND_BLAST_PLUS_ID));
}

#[test]
fn havoc_panacea_plus_grants_two_artifact_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::PANACEA_PLUS_ID),
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
    .expect("Havoc plays Panacea+ from the draw pile");

    assert_eq!(next.player.powers.artifact, 2);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::HAVOC_ID);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::PANACEA_PLUS_ID
    );
}

#[test]
fn havoc_panache_plus_grants_fourteen_damage_power() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::PANACHE_PLUS_ID),
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
    .expect("Havoc plays Panache+ from the draw pile");

    assert_eq!(next.player.powers.panache, 14);
    assert_eq!(next.player.powers.panache_cards_played, 0);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::HAVOC_ID);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::PANACHE_PLUS_ID
    );
}

#[test]
fn havoc_sadistic_nature_plus_grants_seven_damage_power() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::SADISTIC_NATURE_PLUS_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Havoc plays Sadistic Nature+ from the draw pile");

    assert_eq!(next.player.powers.sadistic_nature, 7);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::HAVOC_ID);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::SADISTIC_NATURE_PLUS_ID
    );
}

#[test]
fn panache_counter_resets_at_start_of_player_turn() {
    let mut state = CombatState::initial_fixture();
    state.player.powers.panache = 10;
    state.player.powers.panache_cards_played = 4;
    state.piles.hand.clear();
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(1), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::STRIKE_R_ID),
    ];

    start_player_turn(&mut state);

    assert_eq!(state.player.powers.panache_cards_played, 0);
    assert_eq!(state.piles.hand.len(), 5);
}

#[test]
fn havoc_panic_button_plus_gains_forty_block_and_prevents_block() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::PANIC_BUTTON_PLUS_ID),
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
    .expect("Havoc plays Panic Button+ from the draw pile");

    assert_eq!(next.player.block, 40);
    assert_eq!(next.player.no_block_turns, 2);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::HAVOC_ID);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::PANIC_BUTTON_PLUS_ID
    );
}

#[test]
fn top_draw_purity_plus_exhausts_up_to_five_hand_cards() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::BASH_ID),
        CardInstance::new(CardId::new(4), cards::ANGER_ID),
        CardInstance::new(CardId::new(5), cards::SHRUG_IT_OFF_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(6), cards::PURITY_PLUS_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let mut next = apply_play_top_draw_card_action(&state, None)
        .expect("top-draw Purity+ opens exhaust selection");
    assert!(next.exhaust_select.is_some());

    for ui_index in 0..5 {
        choose_exhaust_select(&mut next, ui_index).expect("select next hand card");
    }
    confirm_exhaust_select(&mut next).expect("confirm Purity+ exhaust selection");

    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 6);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::PURITY_PLUS_ID));
    assert!(next.exhaust_select.is_none());
}

#[test]
fn top_draw_secret_technique_fetches_skill_and_exhausts_source() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand.clear();
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(1), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::SECRET_TECHNIQUE_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let mut next = apply_play_top_draw_card_action(&state, None)
        .expect("top-draw Secret Technique opens draw selection");
    assert!(next.draw_select.is_some());

    choose_draw_select(&mut next, 0).expect("select Defend skill");
    confirm_draw_select(&mut next).expect("confirm Secret Technique draw selection");

    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert!(next.piles.discard_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::SECRET_TECHNIQUE_ID
    );
    assert!(next.draw_select.is_none());
}

#[test]
fn top_draw_secret_weapon_plus_fetches_attack_and_discards_source() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand.clear();
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(1), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::SECRET_WEAPON_PLUS_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let mut next = apply_play_top_draw_card_action(&state, None)
        .expect("top-draw Secret Weapon+ opens draw selection");
    assert!(next.draw_select.is_some());

    choose_draw_select(&mut next, 0).expect("select Strike attack");
    confirm_draw_select(&mut next).expect("confirm Secret Weapon+ draw selection");

    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::SECRET_WEAPON_PLUS_ID
    );
    assert!(next.piles.exhaust_pile.is_empty());
    assert!(next.draw_select.is_none());
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
fn chrysalis_adds_three_generated_skills_to_draw_pile_with_source_cost_rule() {
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
    assert!(next
        .piles
        .draw_pile
        .iter()
        .all(generated_card_uses_source_cost_rule));
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::CHRYSALIS_ID);
}

#[test]
fn havoc_chrysalis_plus_adds_five_generated_skills_and_exhausts() {
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
    let generated_cards = next
        .piles
        .draw_pile
        .iter()
        .filter(|card| card.combat_only)
        .collect::<Vec<_>>();
    assert_eq!(generated_cards.len(), 5);
    let cost_rule_violations = generated_cards
        .iter()
        .filter(|card| !generated_card_uses_source_cost_rule(card))
        .map(|card| {
            let definition = cards::get_card_definition(card.content_id).expect("known card");
            (
                definition.key,
                definition.cost,
                card.temp_cost,
                card.temp_cost_turn_only,
            )
        })
        .collect::<Vec<_>>();
    assert!(
        generated_cards
            .iter()
            .all(|card| generated_card_uses_source_cost_rule(card)),
        "{cost_rule_violations:?}"
    );
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::CHRYSALIS_PLUS_ID
    );
}

fn generated_card_uses_source_cost_rule(card: &CardInstance) -> bool {
    let Some(definition) = cards::get_card_definition(card.content_id) else {
        return false;
    };
    let expected_temp_cost = if definition.cost > 0 { Some(0) } else { None };
    card.combat_only && card.temp_cost == expected_temp_cost && !card.temp_cost_turn_only
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
