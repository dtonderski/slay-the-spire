#![allow(clippy::assertions_on_constants)]

use sts_core::{
    apply_combat_action, apply_combat_action_on_run,
    card::{CardType, TargetRequirement},
    combat::{
        hand::resolve_end_of_turn_hand,
        transition::{
            apply_play_top_draw_card_action, choose_discard_select, choose_draw_select,
            choose_exhaust_select, choose_hand_select, confirm_draw_select, confirm_exhaust_select,
            confirm_hand_select, confirm_headbutt_select,
        },
        turn::{end_player_turn, start_player_turn},
        turn_powers::apply_end_of_player_turn_powers,
    },
    content::{
        cards,
        monsters::{
            monster_state, DARKLING_A0, FIXED_SIMPLE_MONSTER, GUARDIAN_A0, GUARDIAN_DEFENSIVE_BLOCK,
        },
        shop_pool::colorless_discovery_pool,
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState, MonsterId,
    MonsterIntent, Relic, RunPhase, RunState, StsRng,
};

fn valid_legal_combat_actions(state: &CombatState) -> Vec<CombatAction> {
    legal_combat_actions(state).expect("valid combat state")
}

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
fn inert_status_and_curse_keywords_match_source_definitions() {
    assert!(cards::ASCENDERS_BANE.keywords.unplayable);
    assert!(cards::ASCENDERS_BANE.keywords.ethereal);
    assert!(cards::DAZED.keywords.unplayable);
    assert!(cards::DAZED.keywords.ethereal);
    assert!(cards::CLUMSY.keywords.unplayable);
    assert!(cards::CLUMSY.keywords.ethereal);
    assert!(cards::CURSE_OF_THE_BELL.keywords.unplayable);
    assert!(!cards::CURSE_OF_THE_BELL.keywords.ethereal);
    assert!(cards::INJURY.keywords.unplayable);
    assert!(!cards::INJURY.keywords.ethereal);
    assert!(cards::WRITHE.keywords.unplayable);
    assert!(cards::WRITHE.keywords.innate);
}

#[test]
fn upgraded_burn_deals_four_blockable_end_turn_damage_and_discards() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.block = 1;
    let mut burn = CardInstance::new(CardId::new(1), cards::BURN_ID);
    burn.upgrades = 1;
    state.piles.hand = vec![burn];
    state.piles.discard_pile.clear();

    let mut next = state.clone();
    resolve_end_of_turn_hand(&mut next).expect("end-turn hand resolves");

    assert_eq!(next.player.hp, 47);
    assert_eq!(next.player.block, 0);
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::BURN_ID);
    assert_eq!(next.piles.discard_pile[0].upgrades, 1);
}

#[test]
fn decay_deals_two_blockable_end_turn_damage_and_discards() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.block = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DECAY_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let mut next = state.clone();
    resolve_end_of_turn_hand(&mut next).expect("end-turn hand resolves");

    assert_eq!(next.player.hp, 49);
    assert_eq!(next.player.block, 0);
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::DECAY_ID);
}

#[test]
fn regret_loses_hp_equal_to_end_turn_hand_size_and_discards() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::REGRET_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();

    let mut next = state.clone();
    resolve_end_of_turn_hand(&mut next).expect("end-turn hand resolves");

    assert_eq!(next.player.hp, 47);
    assert_eq!(next.piles.hand.len(), 2);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::REGRET_ID);
}

#[test]
fn doubt_and_shame_apply_one_end_turn_debuff_each_and_discard() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::DOUBT_ID),
        CardInstance::new(CardId::new(2), cards::SHAME_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.draw_pile.clear();
    state.monsters.clear();
    state.relics.clear();

    let state = end_player_turn(&state).expect("supported monster intent");

    assert_eq!(state.player.powers.weak, 1);
    assert_eq!(state.player.powers.frail, 1);
}

#[test]
fn normality_in_hand_blocks_fourth_card_play() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    state.relic_counters.cards_played_this_turn = 3;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::NORMALITY_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let legal_actions = valid_legal_combat_actions(&state);
    assert!(!legal_actions.contains(&CombatAction::PlayCard {
        card_id: CardId::new(2),
        target: Some(MonsterId::new(1)),
    }));

    let error = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect_err("Normality blocks the fourth card play while in hand");

    assert!(matches!(error, sts_core::SimError::IllegalAction(_)));
}

#[test]
fn barricade_power_is_idempotent_when_replayed() {
    assert_eq!(cards::BARRICADE.cost, 3);
    assert_eq!(cards::BARRICADE_PLUS.cost, 2);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 5;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::BARRICADE_ID),
        CardInstance::new(CardId::new(2), cards::BARRICADE_PLUS_ID),
    ];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("first Barricade plays");
    assert_eq!(next.player.powers.barricade, 1);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("second Barricade plays without stacking the power");

    assert_eq!(next.player.powers.barricade, 1);
}

#[test]
fn inflame_plus_applies_three_strength_and_removes_power_card() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::INFLAME_PLUS_ID)];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Inflame+ plays");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.player.powers.strength, 3);
    assert!(next.piles.hand.is_empty());
    assert!(next.piles.discard_pile.is_empty());
}

#[test]
fn metallicize_plus_grants_four_end_turn_block() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::METALLICIZE_PLUS_ID,
    )];

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Metallicize+ plays");

    assert_eq!(next.player.powers.metallicize, 4);
    assert_eq!(next.player.block, 0);

    apply_end_of_player_turn_powers(&mut next).expect("end-turn powers resolve");

    assert_eq!(next.player.block, 4);
}

#[test]
fn juggernaut_plus_deals_unmodified_damage_when_block_is_gained() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::JUGGERNAUT_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
    ];
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    state.rng.card_random_rng = StsRng::new(4242);
    let starting_hp = state
        .monsters
        .iter()
        .map(|monster| monster.hp)
        .collect::<Vec<_>>();
    let mut expected_rng = StsRng::new(4242);
    let expected_target_index = expected_rng.random_int(1) as usize;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Juggernaut+ plays");
    assert_eq!(next.player.powers.juggernaut, 7);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Defend plays after Juggernaut+");

    assert_eq!(next.player.block, 5);
    for (index, monster) in next.monsters.iter().enumerate() {
        let expected_damage = if index == expected_target_index { 7 } else { 0 };
        assert_eq!(monster.hp, starting_hp[index] - expected_damage);
    }
    assert_eq!(next.rng.card_random_rng.counter(), expected_rng.counter());
}

#[test]
fn limit_break_plus_doubles_current_strength_without_exhausting() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.powers.strength = 4;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::LIMIT_BREAK_PLUS_ID,
    )];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Limit Break+ plays");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.player.powers.strength, 8);
    assert!(next.piles.exhaust_pile.is_empty());
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::LIMIT_BREAK_PLUS_ID
    );
}

#[test]
fn limit_break_counts_temporary_flex_strength() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.temp_strength = 2;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::LIMIT_BREAK_PLUS_ID,
    )];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Limit Break+ plays after Flex");

    assert_eq!(next.player.powers.strength, 2);
    assert_eq!(next.player.temp_strength, 2);

    let mut after_turn = next.clone();
    start_player_turn(&mut after_turn).expect("player turn starts");
    assert_eq!(after_turn.player.powers.strength, 2);
    assert_eq!(after_turn.player.temp_strength, 0);
}

#[test]
fn offering_plus_loses_six_hp_gains_two_energy_draws_five_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::OFFERING_PLUS_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::ANGER_ID),
        CardInstance::new(CardId::new(6), cards::FLEX_ID),
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
    .expect("Offering+ plays");

    assert_eq!(next.player.hp, 44);
    assert_eq!(next.player.energy, 2);
    assert_eq!(next.piles.hand.len(), 5);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::OFFERING_PLUS_ID
    );
}

#[test]
fn seeing_red_plus_gains_two_energy_and_exhausts_at_zero_cost() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SEEING_RED_PLUS_ID)];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Seeing Red+ plays");

    assert_eq!(next.player.energy, 2);
    assert!(next.piles.discard_pile.is_empty());
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::SEEING_RED_PLUS_ID
    );
}

#[test]
fn power_through_plus_adds_two_generated_wounds_then_gains_twenty_block() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::POWER_THROUGH_PLUS_ID,
    )];
    for id in 2..=10 {
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(id), cards::STRIKE_R_ID));
    }
    state.piles.draw_pile.clear();
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Power Through+ plays");

    assert_eq!(next.player.block, 20);
    let hand_wounds = next
        .piles
        .hand
        .iter()
        .filter(|card| card.content_id == cards::WOUND_ID)
        .collect::<Vec<_>>();
    assert_eq!(hand_wounds.len(), 1);
    assert!(hand_wounds[0].combat_only);
    assert_eq!(next.piles.discard_pile.len(), 2);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::WOUND_ID);
    assert!(next.piles.discard_pile[0].combat_only);
    assert_eq!(
        next.piles.discard_pile[1].content_id,
        cards::POWER_THROUGH_PLUS_ID
    );
}

#[test]
fn power_through_with_nine_card_hand_keeps_both_wounds_before_source_discard() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::POWER_THROUGH_ID)];
    for id in 2..=9 {
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(id), cards::STRIKE_R_ID));
    }
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
    .expect("Power Through plays");

    assert_eq!(next.piles.hand.len(), 10);
    assert_eq!(
        next.piles.hand[8..]
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>(),
        vec![cards::WOUND_ID, cards::WOUND_ID]
    );
    assert!(next.piles.hand[8..].iter().all(|card| card.combat_only));
    assert_eq!(next.piles.hand[8].id, CardId::new(10));
    assert_eq!(next.piles.hand[9].id, CardId::new(11));
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::POWER_THROUGH_ID
    );
}

#[test]
fn rage_plus_grants_five_block_when_next_attack_is_played() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::RAGE_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Rage+ plays");
    assert_eq!(next.player.temp_rage_block, 5);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike plays after Rage+");

    assert_eq!(next.player.block, 5);
    assert_eq!(next.monsters[0].hp, starting_hp - 6);
}

#[test]
fn rampage_plus_uses_and_increases_per_instance_damage_bonus() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    let mut rampage = CardInstance::new(CardId::new(1), cards::RAMPAGE_PLUS_ID);
    rampage.rampage_damage_bonus = 8;
    state.piles.hand = vec![rampage];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Rampage+ plays");

    assert_eq!(next.monsters[0].hp, starting_hp - 16);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::RAMPAGE_PLUS_ID
    );
    assert_eq!(next.piles.discard_pile[0].rampage_damage_bonus, 16);
}

#[test]
fn rampage_damage_overflow_fails_before_resolving_the_hit() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    let mut rampage = CardInstance::new(CardId::new(1), cards::RAMPAGE_ID);
    rampage.rampage_damage_bonus = i32::MAX;
    state.piles.hand = vec![rampage];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    assert_eq!(
        apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(MonsterId::new(1)),
            },
        ),
        Err(sts_core::SimError::InvalidState(
            "Rampage damage overflows i32"
        ))
    );
}

#[test]
fn reaper_plus_heals_for_unblocked_damage_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 40;
    state.player.max_hp = 80;
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::REAPER_PLUS_ID)];
    state.piles.exhaust_pile.clear();
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    state.monsters[0].block = 3;
    let first_hp = state.monsters[0].hp;
    let second_hp = state.monsters[1].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Reaper+ plays");

    assert_eq!(next.monsters[0].hp, first_hp - 2);
    assert_eq!(next.monsters[1].hp, second_hp - 5);
    assert_eq!(next.player.hp, 47);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::REAPER_PLUS_ID);
}

#[test]
fn rupture_plus_grants_strength_when_player_loses_hp_from_card() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.energy = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::RUPTURE_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::BLOODLETTING_ID),
    ];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Rupture+ plays");
    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Bloodletting triggers Rupture+");

    assert_eq!(next.player.hp, 47);
    assert_eq!(next.player.powers.rupture, 2);
    assert_eq!(next.player.powers.strength, 2);
}

#[test]
fn searing_blow_repeated_upgrades_use_source_damage_sequence() {
    let base = CardInstance::new(CardId::new(1), cards::SEARING_BLOW_ID);
    let plus_one = cards::upgrade_card_instance(base)
        .expect("first Searing Blow upgrade is representable")
        .expect("Searing Blow upgrades once");
    let plus_two = cards::upgrade_card_instance(plus_one)
        .expect("second Searing Blow upgrade is representable")
        .expect("Searing Blow upgrades twice");
    assert_eq!(plus_two.content_id, cards::SEARING_BLOW_PLUS_ID);
    assert_eq!(plus_two.searing_blow_upgrades, 2);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![plus_two];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Searing Blow+2 plays");

    assert_eq!(next.monsters[0].hp, starting_hp - 21);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::SEARING_BLOW_PLUS_ID
    );
}

#[test]
fn spot_weakness_plus_grants_strength_only_against_attacking_target() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::SPOT_WEAKNESS_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::SPOT_WEAKNESS_ID),
    ];
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    state.monsters[0].intent = MonsterIntent::Attack { damage: 6 };
    state.monsters[1].intent = MonsterIntent::Block { block: 8 };

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Spot Weakness+ plays against attacking target");
    assert_eq!(next.player.powers.strength, 4);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: Some(MonsterId::new(2)),
        },
    )
    .expect("Spot Weakness plays against non-attacking target");
    assert_eq!(next.player.powers.strength, 4);
}

#[test]
fn spot_weakness_recognizes_attack_intents_with_deferred_status_cards() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SPOT_WEAKNESS_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    for intent in [
        MonsterIntent::AddBurnToDiscard {
            count: 1,
            damage: 6,
        },
        MonsterIntent::AttackMultipleUpgradeBurns {
            damage: 2,
            hits: 6,
            count: 1,
        },
        // Awakened One phase-2 Sludge (CM ATTACK_DEBUFF). FIDL00221 step 1595.
        MonsterIntent::AttackAddVoidToDraw {
            damage: 18,
            count: 1,
        },
    ] {
        state.player.powers.strength = 0;
        state.monsters[0].intent = intent;
        state.piles.hand[0] = CardInstance::new(CardId::new(1), cards::SPOT_WEAKNESS_ID);
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Spot Weakness plays against a deferred-status attack");
        assert_eq!(next.player.powers.strength, 3);
    }
}

#[test]
fn thunderclap_plus_deals_damage_then_applies_one_vulnerable_to_all_enemies() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::THUNDERCLAP_PLUS_ID,
    )];
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    let starting_hp = state
        .monsters
        .iter()
        .map(|monster| monster.hp)
        .collect::<Vec<_>>();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Thunderclap+ plays");

    assert_eq!(next.monsters[0].hp, starting_hp[0] - 7);
    assert_eq!(next.monsters[1].hp, starting_hp[1] - 7);
    assert!(next
        .monsters
        .iter()
        .all(|monster| monster.powers.vulnerable == 1));
}

#[test]
fn uppercut_plus_deals_damage_then_applies_two_weak_and_vulnerable() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::UPPERCUT_PLUS_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Uppercut+ plays");

    assert_eq!(next.monsters[0].hp, starting_hp - 13);
    assert_eq!(next.monsters[0].powers.weak, 2);
    assert_eq!(next.monsters[0].powers.vulnerable, 2);
}

#[test]
fn true_grit_plus_gains_block_then_exhausts_selected_card() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::TRUE_GRIT_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.exhaust_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("True Grit+ opens exhaust selection");
    assert_eq!(next.player.block, 9);
    assert!(next.exhaust_select().is_some());

    choose_exhaust_select(&mut next, 1).expect("select Defend");
    confirm_exhaust_select(&mut next).expect("confirm True Grit+ selection");

    assert!(next.exhaust_select().is_none());
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::TRUE_GRIT_PLUS_ID
    );
}

#[test]
fn true_grit_random_exhaust_skips_rng_when_one_card_is_eligible() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::TRUE_GRIT_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
    ];
    state.piles.exhaust_pile.clear();
    let starting_card_rng_counter = state.rng.card_random_rng.counter();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("True Grit should exhaust the only eligible card");

    assert_eq!(
        next.rng.card_random_rng.counter(),
        starting_card_rng_counter
    );
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::TRUE_GRIT_ID);
}

#[test]
fn warcry_plus_puts_selected_card_on_top_of_draw() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::WARCRY_PLUS_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.exhaust_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Warcry+ draws before choosing a card");

    assert!(next.hand_select().is_some());
    let selected_content_id = next
        .piles
        .hand
        .iter()
        .find(|card| card.id != CardId::new(1))
        .expect("drawn card is selectable")
        .content_id;
    choose_hand_select(&mut next, 0).expect("select drawn card");
    confirm_hand_select(&mut next).expect("confirm Warcry+ selection");

    assert!(next.hand_select().is_none());
    assert_eq!(
        next.piles
            .draw_pile
            .last()
            .expect("selected card returns to draw pile")
            .content_id,
        selected_content_id
    );
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::WARCRY_PLUS_ID);
}

#[test]
fn warcry_with_dark_embrace_draws_the_card_put_on_top() {
    // PutOnDeckAction finishes before Warcry exhausts. Dark Embrace must
    // therefore draw the just-selected top card (e.g. Havoc) rather than the
    // previous top of the draw pile.
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.player.powers.dark_embrace = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::WARCRY_ID),
        CardInstance::new(CardId::new(2), cards::HAVOC_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(4), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(5), cards::BASH_ID),
    ];
    state.piles.exhaust_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Warcry opens hand select after drawing");
    // After the Warcry draw, hand is [Warcry, Havoc, Strike, Bash]; UI skips
    // the source, so index 0 is Havoc.
    choose_hand_select(&mut next, 0).expect("select Havoc");
    confirm_hand_select(&mut next).expect("confirm Warcry selection");

    assert!(
        next.piles
            .hand
            .iter()
            .any(|card| card.content_id == cards::HAVOC_ID),
        "Dark Embrace must draw the Warcry-selected top card: hand={:?}",
        next.piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>()
    );
    assert!(
        !next
            .piles
            .draw_pile
            .iter()
            .any(|card| card.content_id == cards::HAVOC_ID),
        "selected Havoc must not remain buried under a later put-on-deck"
    );
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::WARCRY_ID);
    assert_eq!(
        next.piles.draw_pile.last().map(|card| card.content_id),
        Some(cards::DEFEND_R_ID),
        "only the pre-existing non-top cards remain in the draw pile"
    );
}

#[test]
fn warcry_with_no_card_after_draw_exhausts_without_opening_selection() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::WARCRY_ID)];
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
    .expect("Warcry plays with no card to put back");

    assert!(next.hand_select().is_none());
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::WARCRY_ID);
}

#[test]
fn warcry_with_only_drawn_card_auto_puts_on_deck_without_selection() {
    // PutOnDeckAction opens HandCardSelectScreen only when hand.size() > amount.
    // After Warcry draws into an otherwise empty hand, size==amount==1 so the
    // drawn card is auto-placed via cardRandomRng (including random(0) for a
    // singleton) and no player HandSelect remains.
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    let rng_before = state.rng.card_random_rng.counter();
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::WARCRY_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID), // top / drawn
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
    .expect("Warcry auto-completes put-on-deck for a singleton hand");

    assert!(
        next.hand_select().is_none(),
        "singleton post-draw hand must not open HandSelect"
    );
    assert!(
        next.piles.hand.is_empty(),
        "drawn card returns to draw pile"
    );
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::WARCRY_ID);
    assert_eq!(
        next.piles.draw_pile.last().map(|card| card.content_id),
        Some(cards::STRIKE_R_ID),
        "auto-placed card sits on top of draw"
    );
    assert_eq!(
        next.piles
            .draw_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>(),
        vec![cards::DEFEND_R_ID, cards::STRIKE_R_ID],
        "net draw then put-back leaves draw order unchanged"
    );
    assert_eq!(
        next.rng.card_random_rng.counter(),
        rng_before + 1,
        "getRandomCard still advances cardRandomRng for size==1"
    );
    assert!(
        valid_legal_combat_actions(&next)
            .iter()
            .any(|action| matches!(action, CombatAction::EndTurn)),
        "END must be legal after auto put-on-deck Warcry"
    );
}

#[test]
fn thinking_ahead_with_no_other_card_draws_without_putting_card_back() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::THINKING_AHEAD_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
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
    .expect("Thinking Ahead plays without a pre-existing card to put back");

    assert!(next.hand_select().is_none());
    assert_eq!(next.piles.hand.len(), 2);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::THINKING_AHEAD_ID
    );
}

#[test]
fn thinking_ahead_plus_with_no_other_card_draws_and_discards_without_selection() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::THINKING_AHEAD_PLUS_ID,
    )];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
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
    .expect("Thinking Ahead+ plays without a pre-existing card to put back");

    assert!(next.hand_select().is_none());
    assert_eq!(next.piles.hand.len(), 2);
    assert!(next.piles.draw_pile.is_empty());
    assert!(next.piles.exhaust_pile.is_empty());
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::THINKING_AHEAD_PLUS_ID
    );
}

#[test]
fn thinking_ahead_plus_can_put_a_drawn_card_on_top_when_pre_hand_was_nonempty() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::THINKING_AHEAD_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::BASH_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Thinking Ahead+ opens put-on-draw selection after drawing");

    assert!(next.hand_select().is_some());
    let selected_index = next
        .piles
        .hand
        .iter()
        .position(|card| card.content_id == cards::DEFEND_R_ID)
        .expect("drawn Defend is selectable");
    let selected_ui_index = next
        .piles
        .hand
        .iter()
        .take(selected_index)
        .filter(|card| card.id != CardId::new(1))
        .count();
    choose_hand_select(&mut next, selected_ui_index).expect("select drawn Defend");
    confirm_hand_select(&mut next).expect("confirm Thinking Ahead+ selection");

    assert!(next.hand_select().is_none());
    assert!(
        next.piles
            .draw_pile
            .iter()
            .any(|card| card.content_id == cards::DEFEND_R_ID),
        "selected drawn card returns to the draw pile"
    );
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::THINKING_AHEAD_PLUS_ID
    );
}

#[test]
fn swift_strike_plus_is_zero_cost_strike_damage() {
    assert_eq!(cards::SWIFT_STRIKE.cost, 0);
    assert_eq!(cards::SWIFT_STRIKE.values.damage, Some(7));
    assert_eq!(cards::SWIFT_STRIKE_PLUS.cost, 0);
    assert_eq!(cards::SWIFT_STRIKE_PLUS.values.damage, Some(10));

    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::SWIFT_STRIKE_PLUS_ID,
    )];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Swift Strike+ plays for zero energy");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_hp - 10);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::SWIFT_STRIKE_PLUS_ID
    );
}

#[test]
fn the_bomb_plus_arms_three_turn_fifty_damage_timer() {
    assert_eq!(cards::THE_BOMB.cost, 2);
    assert_eq!(cards::THE_BOMB.values.damage, Some(40));
    assert_eq!(cards::THE_BOMB_PLUS.cost, 2);
    assert_eq!(cards::THE_BOMB_PLUS.values.damage, Some(50));

    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::THE_BOMB_PLUS_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    state.monsters[0].hp = 60;
    state.monsters[0].max_hp = 60;
    state.monsters[1].hp = 70;
    state.monsters[1].max_hp = 70;

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("The Bomb+ arms a timer");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.bomb_timers.len(), 1);
    assert_eq!(next.bomb_timers[0].turns_remaining, 3);
    assert_eq!(next.bomb_timers[0].damage, 50);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::THE_BOMB_PLUS_ID
    );

    apply_end_of_player_turn_powers(&mut next).expect("end-turn powers resolve");
    assert_eq!(next.bomb_timers[0].turns_remaining, 2);
    assert_eq!(next.monsters[0].hp, 60);
    assert_eq!(next.monsters[1].hp, 70);

    apply_end_of_player_turn_powers(&mut next).expect("end-turn powers resolve");
    assert_eq!(next.bomb_timers[0].turns_remaining, 1);
    assert_eq!(next.monsters[0].hp, 60);
    assert_eq!(next.monsters[1].hp, 70);

    apply_end_of_player_turn_powers(&mut next).expect("end-turn powers resolve");
    assert!(next.bomb_timers.is_empty());
    assert_eq!(next.monsters[0].hp, 10);
    assert_eq!(next.monsters[1].hp, 20);
}

#[test]
fn violence_plus_draws_four_attacks_from_draw_pile_and_exhausts() {
    assert_eq!(cards::VIOLENCE.cost, 0);
    assert!(cards::VIOLENCE.keywords.exhaust);
    assert_eq!(cards::VIOLENCE_PLUS.cost, 0);
    assert!(cards::VIOLENCE_PLUS.keywords.exhaust);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::VIOLENCE_PLUS_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::SWIFT_STRIKE_ID),
        CardInstance::new(CardId::new(6), cards::POMMEL_STRIKE_ID),
    ];
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Violence+ draws attacks from draw pile");

    let hand_content_ids = next
        .piles
        .hand
        .iter()
        .map(|card| card.content_id)
        .collect::<Vec<_>>();
    assert_eq!(hand_content_ids.len(), 4);
    assert!(hand_content_ids.contains(&cards::STRIKE_R_ID));
    assert!(hand_content_ids.contains(&cards::BASH_ID));
    assert!(hand_content_ids.contains(&cards::SWIFT_STRIKE_ID));
    assert!(hand_content_ids.contains(&cards::POMMEL_STRIKE_ID));
    assert_eq!(next.piles.draw_pile[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::VIOLENCE_PLUS_ID
    );
}

#[test]
fn armaments_plus_upgrades_all_other_hand_cards_without_selection() {
    assert_eq!(cards::ARMAMENTS.values.block, Some(5));
    assert_eq!(cards::ARMAMENTS_PLUS.values.block, Some(5));

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::ARMAMENTS_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Armaments+ plays without opening hand selection");

    assert_eq!(next.player.block, 5);
    assert!(next.hand_select().is_none());
    assert_eq!(next.piles.hand[0].content_id, cards::STRIKE_R_PLUS_ID);
    assert_eq!(next.piles.hand[1].content_id, cards::DEFEND_R_PLUS_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::ARMAMENTS_PLUS_ID
    );
}

#[test]
fn armaments_plus_adjusts_confused_cost_when_upgrade_reduces_base_cost() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    let mut armaments = CardInstance::new(CardId::new(1), cards::ARMAMENTS_PLUS_ID);
    armaments.temp_cost = Some(0);
    let mut defend = CardInstance::new(CardId::new(2), cards::DEFEND_R_ID);
    defend.temp_cost = Some(2);
    let mut seeing_red = CardInstance::new(CardId::new(3), cards::SEEING_RED_ID);
    seeing_red.temp_cost = Some(1);
    state.piles.hand = vec![armaments, defend, seeing_red];
    state.piles.discard_pile.clear();

    let after_armaments = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("zero-cost Armaments+ plays");

    assert_eq!(after_armaments.player.energy, 3);
    assert_eq!(
        after_armaments.piles.hand[0].content_id,
        cards::DEFEND_R_PLUS_ID
    );
    assert_eq!(after_armaments.piles.hand[0].temp_cost, Some(2));
    assert_eq!(
        after_armaments.piles.hand[1].content_id,
        cards::SEEING_RED_PLUS_ID
    );
    assert_eq!(after_armaments.piles.hand[1].temp_cost, Some(0));

    let after_defend = apply_combat_action(
        &after_armaments,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Confusion-cost Defend+ plays");
    assert_eq!(after_defend.player.energy, 1);

    let after_seeing_red = apply_combat_action(
        &after_defend,
        CombatAction::PlayCard {
            card_id: CardId::new(3),
            target: None,
        },
    )
    .expect("Armaments-upgraded Seeing Red+ plays for zero");
    assert_eq!(after_seeing_red.player.energy, 3);
}

#[test]
fn anger_plus_adds_generated_stat_equivalent_copy_before_source_discard() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::ANGER_PLUS_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Anger+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 8);
    assert_eq!(next.piles.discard_pile.len(), 2);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::ANGER_PLUS_ID);
    assert!(!next.piles.discard_pile[0].combat_only);
    assert_eq!(next.piles.discard_pile[1].content_id, cards::ANGER_PLUS_ID);
    assert!(!next.piles.discard_pile[1].combat_only);
}

#[test]
fn berserk_plus_applies_one_vulnerable_and_one_berserk() {
    assert_eq!(cards::BERSERK.values.vulnerable, Some(2));
    assert_eq!(cards::BERSERK_PLUS.values.vulnerable, Some(1));

    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BERSERK_PLUS_ID)];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Berserk+ plays without a target");

    assert_eq!(next.player.powers.vulnerable, 1);
    assert_eq!(next.player.powers.berserk, 1);
}

#[test]
fn bash_plus_deals_damage_then_applies_three_vulnerable() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BASH_PLUS_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Bash+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 10);
    assert_eq!(next.monsters[0].powers.vulnerable, 3);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::BASH_PLUS_ID);
}

#[test]
fn bloodletting_plus_loses_hp_reduces_blood_for_blood_and_gains_three_energy() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.player.hp = 50;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::BLOODLETTING_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::BLOOD_FOR_BLOOD_ID),
    ];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Bloodletting+ plays without a target");

    assert_eq!(next.player.hp, 47);
    assert_eq!(next.player.energy, 3);
    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::BLOOD_FOR_BLOOD_ID);
    assert_eq!(next.piles.hand[0].blood_for_blood_cost_reduction, 1);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Blood for Blood spends reduced cost after Bloodletting HP loss");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_hp - 18);
    assert_eq!(next.piles.discard_pile.len(), 2);
}

#[test]
fn blood_for_blood_upgrade_preserves_combat_cost_reduction() {
    let mut card = CardInstance::new(CardId::new(1), cards::BLOOD_FOR_BLOOD_ID);
    card.blood_for_blood_cost_reduction = 2;

    let upgraded = cards::upgrade_card_instance(card)
        .expect("Blood for Blood upgrade is representable")
        .expect("Blood for Blood upgrades");

    assert_eq!(upgraded.content_id, cards::BLOOD_FOR_BLOOD_PLUS_ID);
    assert_eq!(upgraded.blood_for_blood_cost_reduction, 2);
}

#[test]
fn fully_reduced_blood_for_blood_is_legal_and_spends_zero_energy() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    let mut card = CardInstance::new(CardId::new(1), cards::BLOOD_FOR_BLOOD_ID);
    card.blood_for_blood_cost_reduction = 4;
    state.piles.hand = vec![card];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let action = CombatAction::PlayCard {
        card_id: card.id,
        target: Some(MonsterId::new(1)),
    };
    assert!(valid_legal_combat_actions(&state).contains(&action));
    let next = apply_combat_action(&state, action).expect("zero-cost Blood for Blood plays");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.discard_pile[0], card);
}

#[test]
fn body_slam_plus_deals_current_block_as_attack_damage() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.player.block = 17;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BODY_SLAM_PLUS_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Body Slam+ plays for current block damage");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_hp - 17);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::BODY_SLAM_PLUS_ID
    );
}

#[test]
fn body_slam_with_zero_block_applies_vigor_before_vulnerable() {
    // Trace pattern: Body Slam, block=0, Vigor=8, target Vulnerable → 12 damage.
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.player.powers.vigor = 8;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BODY_SLAM_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].powers.vulnerable = 1;
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Body Slam with Vigor plays");

    // (0 block + 8 vigor) * 1.5 vulnerable = 12
    assert_eq!(next.monsters[0].hp, starting_hp - 12);
    assert_eq!(next.player.powers.vigor, 0);
}

#[test]
fn brutality_loses_one_hp_then_draws_before_normal_turn_draw() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.powers.brutality = 1;
    state.piles.hand.clear();
    state.piles.draw_pile = (1..=6)
        .map(|id| CardInstance::new(CardId::new(id), cards::STRIKE_R_ID))
        .collect();
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    start_player_turn(&mut state).expect("player turn starts");

    assert_eq!(state.player.hp, 49);
    assert_eq!(state.piles.hand.len(), 6);
    assert!(state.piles.draw_pile.is_empty());
}

#[test]
fn burning_pact_plus_exhausts_one_other_card_then_draws_three() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::BURNING_PACT_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::ANGER_ID),
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
    .expect("Burning Pact+ exhausts the only other hand card automatically");

    assert_eq!(next.player.energy, 0);
    assert!(next.exhaust_select().is_none());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.hand.len(), 3);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::BURNING_PACT_PLUS_ID
    );
}

#[test]
fn clash_requires_every_card_in_hand_to_be_an_attack() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::CLASH_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    assert!(
        !valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        })
    );

    state.piles.hand[1] = CardInstance::new(CardId::new(2), cards::STRIKE_R_ID);

    assert!(
        valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        })
    );
}

#[test]
fn cleave_plus_deals_eleven_damage_to_all_living_enemies() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::CLEAVE_PLUS_ID)];
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    let starting_hp = state
        .monsters
        .iter()
        .map(|monster| monster.hp)
        .collect::<Vec<_>>();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Cleave+ plays without a target");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_hp[0] - 11);
    assert_eq!(next.monsters[1].hp, starting_hp[1] - 11);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::CLEAVE_PLUS_ID);
}

#[test]
fn clothesline_plus_deals_damage_then_applies_three_weak() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::CLOTHESLINE_PLUS_ID,
    )];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Clothesline+ plays against an enemy");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_hp - 14);
    assert_eq!(next.monsters[0].powers.weak, 3);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::CLOTHESLINE_PLUS_ID
    );
}

#[test]
fn combust_stacks_hp_loss_count_and_damage_amount() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::COMBUST_ID),
        CardInstance::new(CardId::new(2), cards::COMBUST_PLUS_ID),
    ];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Combust plays");
    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Combust+ stacks");

    assert_eq!(next.player.powers.combust, 2);
    assert_eq!(next.player.powers.combust_damage, 12);
}

#[test]
fn corruption_power_is_idempotent_when_replayed() {
    assert_eq!(cards::CORRUPTION.cost, 3);
    assert_eq!(cards::CORRUPTION_PLUS.cost, 2);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 5;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::CORRUPTION_ID),
        CardInstance::new(CardId::new(2), cards::CORRUPTION_PLUS_ID),
    ];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Corruption plays");
    assert_eq!(next.player.powers.corruption, 1);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Corruption+ replays without stacking");

    assert_eq!(next.player.powers.corruption, 1);
}

#[test]
fn corruption_makes_skills_free_and_exhaust_on_use() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.player.powers.corruption = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DEFEND_R_ID)];
    state.piles.exhaust_pile.clear();

    assert!(
        valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
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
    .expect("Corruption makes Defend free");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.player.block, 5);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::DEFEND_R_ID);
}

#[test]
fn dark_embrace_stacks_one_draw_per_exhaust() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::DARK_EMBRACE_ID),
        CardInstance::new(CardId::new(2), cards::DARK_EMBRACE_PLUS_ID),
    ];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Dark Embrace plays");
    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Dark Embrace+ stacks");

    assert_eq!(next.player.powers.dark_embrace, 2);
}

#[test]
fn dark_embrace_end_turn_draws_after_hand_discard() {
    let mut state = CombatState::initial_fixture();
    state.player.powers.dark_embrace = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::PERFECTED_STRIKE_ID),
        CardInstance::new(CardId::new(2), cards::DAZED_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(6), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(7), cards::BASH_ID),
        CardInstance::new(CardId::new(8), cards::STRIKE_R_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = end_player_turn(&state).expect("supported monster intent");

    assert_eq!(
        next.piles
            .discard_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>(),
        vec![cards::PERFECTED_STRIKE_ID]
    );
    assert_eq!(
        next.piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>(),
        vec![
            cards::STRIKE_R_ID,
            cards::BASH_ID,
            cards::DEFEND_R_ID,
            cards::STRIKE_R_ID,
            cards::BASH_ID,
            cards::DEFEND_R_ID,
        ]
    );
}

#[test]
fn dark_embrace_end_turn_draw_ignores_expired_no_draw_power() {
    let mut state = CombatState::initial_fixture();
    state.player.cannot_draw = true;
    state.player.powers.dark_embrace = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::CARNAGE_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(6), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(7), cards::BASH_ID),
    ];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = end_player_turn(&state).expect("supported monster intent");

    assert!(!next.player.cannot_draw);
    assert_eq!(next.piles.hand.len(), 6);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::CARNAGE_ID);
}

#[test]
fn perfected_strike_plus_counts_hand_draw_and_discard_strikes_not_exhaust() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::PERFECTED_STRIKE_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), cards::POMMEL_STRIKE_ID)];
    state.piles.discard_pile = vec![CardInstance::new(CardId::new(4), cards::STRIKE_R_ID)];
    state.piles.exhaust_pile = vec![CardInstance::new(CardId::new(5), cards::STRIKE_R_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Perfected Strike+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 18);
    assert_eq!(
        next.piles.discard_pile[1].content_id,
        cards::PERFECTED_STRIKE_PLUS_ID
    );
}

#[test]
fn pommel_strike_plus_deals_ten_and_draws_two() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::POMMEL_STRIKE_PLUS_ID,
    )];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Pommel Strike+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 10);
    assert_eq!(next.piles.hand.len(), 2);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::POMMEL_STRIKE_PLUS_ID
    );
}

#[test]
fn strike_dummy_boosts_pommel_strike() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.relics = vec![Relic::StrikeDummy];
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::POMMEL_STRIKE_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Pommel Strike plays with Strike Dummy");

    assert_eq!(next.monsters[0].hp, starting_hp - 12);
}

#[test]
fn pommel_strike_draws_after_freeing_a_full_hand_slot() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = (1..=9)
        .map(|id| CardInstance::new(CardId::new(id), cards::STRIKE_R_ID))
        .chain(std::iter::once(CardInstance::new(
            CardId::new(10),
            cards::POMMEL_STRIKE_ID,
        )))
        .collect();
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), cards::DEFEND_R_ID)];
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(10),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Pommel Strike should play from a full hand");

    assert_eq!(next.piles.hand.len(), 10);
    assert!(next
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == cards::DEFEND_R_ID));
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::POMMEL_STRIKE_ID
    );
}

#[test]
fn master_of_strategy_draws_after_freeing_a_full_hand_slot() {
    // Master of Strategy draws 3 while the played card is in limbo. At max hand
    // size the freed slot allows one draw before the remaining draws cap out;
    // plain DrawCards would skip all three and leave the hand one short.
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = (1..=9)
        .map(|id| CardInstance::new(CardId::new(id), cards::STRIKE_R_ID))
        .chain(std::iter::once(CardInstance::new(
            CardId::new(10),
            cards::MASTER_OF_STRATEGY_ID,
        )))
        .collect();
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(13), cards::BASH_ID),
        CardInstance::new(CardId::new(12), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(11), cards::POMMEL_STRIKE_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(10),
            target: None,
        },
    )
    .expect("Master of Strategy should play from a full hand");

    assert_eq!(next.piles.hand.len(), 10);
    assert_eq!(
        next.piles.hand.last().map(|card| card.content_id),
        Some(cards::POMMEL_STRIKE_ID),
        "the single successful draw fills the freed hand slot"
    );
    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::MASTER_OF_STRATEGY_ID
    );
}

#[test]
fn pommel_strike_double_tap_keeps_source_available_for_copied_draw() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.double_tap_pending = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::POMMEL_STRIKE_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(3),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Double Tap should copy Pommel Strike's draw");

    assert_eq!(next.piles.hand.len(), 4);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::POMMEL_STRIKE_ID
    );
}

#[test]
fn double_tap_headbutt_copies_discard_to_draw_effect() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.double_tap_pending = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HEADBUTT_ID)];
    state.piles.draw_pile.clear();
    state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), cards::DEFEND_R_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Double Tap should copy Headbutt's discard effect");

    assert!(next.discard_select().is_none());
    assert_eq!(
        next.piles
            .draw_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>(),
        vec![cards::DEFEND_R_ID, cards::HEADBUTT_ID]
    );
    assert!(next.piles.discard_pile.is_empty());
}

#[test]
fn unused_duplication_potion_stack_expires_at_end_of_round() {
    let mut state = CombatState::initial_fixture();
    state.duplication_potion_stacks = 2;
    state.piles.discard_pile.clear();

    let next = end_player_turn(&state).expect("end turn should resolve");

    assert_eq!(next.duplication_potion_stacks, 1);
    assert!(!next.duplication_potion_pending);
}

#[test]
fn defend_plus_gains_eight_block_and_discards() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DEFEND_R_PLUS_ID)];
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Defend+ plays");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.player.block, 8);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::DEFEND_R_PLUS_ID
    );
}

#[test]
fn double_tap_plus_doubles_the_next_attack_and_leaves_one_pending() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::DOUBLE_TAP_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Double Tap+ plays");
    assert_eq!(next.double_tap_pending, 2);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike is doubled by Double Tap+");

    assert_eq!(next.double_tap_pending, 1);
    assert_eq!(next.monsters[0].hp, starting_hp - 12);
}

#[test]
fn double_tap_copy_triggers_pain_for_copied_attack() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    state.player.hp = 50;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::DOUBLE_TAP_ID),
        CardInstance::new(CardId::new(2), cards::BASH_ID),
        CardInstance::new(CardId::new(3), cards::PAIN_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Double Tap plays");
    assert_eq!(next.player.hp, 49);

    let next = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Bash is doubled by Double Tap");

    assert_eq!(next.player.hp, 47);
}

#[test]
fn pen_nib_doubles_only_the_original_double_tapped_bash() {
    let target = MonsterId::new(1);
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.player.powers.strength = 3;
    state.relics = vec![Relic::PenNib];
    state.relic_counters.pen_nib_attacks_played = 9;
    state.double_tap_pending = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BASH_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, target)];
    state.monsters[0].hp = 46;
    state.monsters[0].max_hp = 46;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(target),
        },
    )
    .expect("Double Tap and Pen Nib Bash should play");

    // Original: (8 + 3) * 2 = 22. Copy after Vulnerable: floor(11 * 1.5) = 16.
    assert_eq!(next.monsters[0].hp, 8);
    assert_eq!(next.monsters[0].powers.vulnerable, 4);
    assert_eq!(next.relic_counters.pen_nib_attacks_played, 1);
}

#[test]
fn pen_nib_doubles_body_slam_including_strength() {
    // FIDL00221 step 1609: 10th attack is Body Slam with 0 block + Str 11 → 22.
    let target = MonsterId::new(1);
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.player.powers.strength = 11;
    state.relics = vec![Relic::PenNib];
    state.relic_counters.pen_nib_attacks_played = 9;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BODY_SLAM_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, target)];
    state.monsters[0].hp = 143;
    state.monsters[0].max_hp = 143;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(target),
        },
    )
    .expect("Pen Nib Body Slam should play");

    assert_eq!(next.monsters[0].hp, 121);
    assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
}

#[test]
fn guardian_mode_shift_reaction_resolves_before_double_tap_copy() {
    let target = MonsterId::new(1);
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.double_tap_pending = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::STRIKE_R_ID)];
    state.monsters = vec![monster_state(&GUARDIAN_A0, target)];
    state.monsters[0].hp = 40;
    state.monsters[0].mode_shift = 1;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(target),
        },
    )
    .expect("Double Tap Strike should play");

    // The original Strike crosses Mode Shift; the copied Strike then hits the
    // defensive block queued at the copied-card action boundary.
    assert_eq!(next.monsters[0].hp, 34);
    assert_eq!(next.monsters[0].block, GUARDIAN_DEFENSIVE_BLOCK - 6);
    assert!(next.monsters[0].in_defensive_mode);
}

#[test]
fn guardian_mode_shift_block_resolves_after_double_tapped_sword_boomerang() {
    let target = MonsterId::new(1);
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.player.powers.strength = 3;
    state.double_tap_pending = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SWORD_BOOMERANG_ID)];
    state.monsters = vec![monster_state(&GUARDIAN_A0, target)];
    state.monsters[0].hp = 141;
    state.monsters[0].powers.vulnerable = 2;
    state.monsters[0].mode_shift = 40;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Double Tap Sword Boomerang should play");

    // Six 9-damage hits land before Guardian enters defensive mode.
    assert_eq!(next.monsters[0].hp, 87);
    assert_eq!(next.monsters[0].block, 20);
    assert!(next.monsters[0].in_defensive_mode);
}

#[test]
fn dual_wield_plus_creates_two_temporary_copies_and_discards_source() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::DUAL_WIELD_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_PLUS_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Dual Wield+ opens hand selection");
    assert!(next.hand_select().is_some());

    choose_hand_select(&mut next, 0).expect("select upgraded Strike");
    confirm_hand_select(&mut next).expect("confirm Dual Wield+ selection");

    assert_eq!(next.player.energy, 0);
    assert!(next.hand_select().is_none());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::DUAL_WIELD_PLUS_ID
    );
    assert!(next.piles.exhaust_pile.is_empty());

    let strike_plus_cards = next
        .piles
        .hand
        .iter()
        .filter(|card| card.content_id == cards::STRIKE_R_PLUS_ID)
        .collect::<Vec<_>>();
    assert_eq!(strike_plus_cards.len(), 3);
    assert_eq!(
        strike_plus_cards
            .iter()
            .filter(|card| card.combat_only)
            .count(),
        2
    );
}

#[test]
fn evolve_plus_draws_two_extra_cards_when_status_is_drawn() {
    let mut state = CombatState::initial_fixture();
    state.player.powers.evolve = 2;
    state.piles.hand.clear();
    state.piles.discard_pile.clear();
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(1), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(6), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(7), cards::WOUND_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    start_player_turn(&mut state).expect("player turn starts");

    assert_eq!(state.piles.hand.len(), 7);
    assert!(state
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == cards::WOUND_ID));
    assert!(state.piles.draw_pile.is_empty());
}

#[test]
fn feel_no_pain_plus_grants_four_block_when_a_card_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.player.powers.feel_no_pain = 4;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SLIMED_ID)];
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Slimed exhausts itself");

    assert_eq!(next.player.block, 4);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::SLIMED_ID);
}

#[test]
fn second_wind_plus_gains_block_once_per_exhausted_non_attack_card() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.block = 0;
    state.player.powers.juggernaut = 5;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::SECOND_WIND_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::BASH_ID),
        CardInstance::new(CardId::new(4), cards::WOUND_ID),
    ];
    state.piles.draw_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Second Wind+ plays");

    assert_eq!(next.player.block, 14);
    assert_eq!(next.monsters[0].hp, starting_hp - 10);
    assert_eq!(
        next.piles
            .exhaust_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>(),
        vec![cards::DEFEND_R_ID, cards::WOUND_ID]
    );
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::SECOND_WIND_PLUS_ID
    );
}

#[test]
fn sever_soul_plus_exhausts_non_attacks_before_dealing_damage() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.player.powers.feel_no_pain = 4;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::SEVER_SOUL_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::BASH_ID),
        CardInstance::new(CardId::new(4), cards::WOUND_ID),
    ];
    state.piles.draw_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Sever Soul+ plays");

    assert_eq!(next.player.block, 8);
    assert_eq!(next.monsters[0].hp, starting_hp - 22);
    assert_eq!(
        next.piles
            .exhaust_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>(),
        vec![cards::DEFEND_R_ID, cards::WOUND_ID]
    );
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::SEVER_SOUL_PLUS_ID
    );
}

#[test]
fn shockwave_plus_applies_five_weak_and_vulnerable_to_all_enemies_and_exhausts() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::SHOCKWAVE_PLUS_ID)];
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
    .expect("Shockwave+ plays");

    assert!(next
        .monsters
        .iter()
        .all(|monster| monster.powers.weak == 5 && monster.powers.vulnerable == 5));
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::SHOCKWAVE_PLUS_ID
    );
}

#[test]
fn fire_breathing_plus_damages_all_enemies_when_status_is_drawn() {
    let mut state = CombatState::initial_fixture();
    state.player.powers.fire_breathing = 10;
    state.piles.hand.clear();
    state.piles.discard_pile.clear();
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), cards::WOUND_ID)];
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    let starting_hp = state
        .monsters
        .iter()
        .map(|monster| monster.hp)
        .collect::<Vec<_>>();

    start_player_turn(&mut state).expect("player turn starts");

    assert_eq!(state.monsters[0].hp, starting_hp[0] - 10);
    assert_eq!(state.monsters[1].hp, starting_hp[1] - 10);
    assert_eq!(state.piles.hand.len(), 1);
    assert_eq!(state.piles.hand[0].content_id, cards::WOUND_ID);
}

#[test]
fn flame_barrier_plus_gains_sixteen_block_and_six_temporary_thorns() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.player.block = 0;
    state.player.temp_thorns = 0;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::FLAME_BARRIER_PLUS_ID,
    )];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Flame Barrier+ plays without a target");

    assert_eq!(next.player.block, 16);
    assert_eq!(next.player.temp_thorns, 6);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::FLAME_BARRIER_PLUS_ID
    );
}

#[test]
fn flex_plus_grants_four_temporary_strength() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.player.temp_strength = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::FLEX_PLUS_ID)];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Flex+ plays without a target");

    assert_eq!(next.player.temp_strength, 4);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::FLEX_PLUS_ID);
}

#[test]
fn artifact_blocks_flex_strength_loss_and_makes_strength_permanent() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.player.powers.artifact = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::FLEX_ID)];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Flex plays with Artifact");

    assert_eq!(next.player.powers.artifact, 0);
    assert_eq!(next.player.powers.strength, 2);
    assert_eq!(next.player.temp_strength, 0);

    let after_turn = apply_combat_action(&next, CombatAction::EndTurn).expect("turn ends");
    assert_eq!(after_turn.player.powers.strength, 2);
}

#[test]
fn artifact_gained_after_flex_blocks_its_end_of_turn_strength_loss() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FLEX_ID),
        CardInstance::new(CardId::new(2), cards::PANACEA_ID),
    ];

    let after_flex = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Flex plays before Artifact is gained");
    let after_panacea = apply_combat_action(
        &after_flex,
        CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        },
    )
    .expect("Panacea grants Artifact after Flex");
    let after_turn = apply_combat_action(&after_panacea, CombatAction::EndTurn).expect("turn ends");

    assert_eq!(after_turn.player.powers.strength, 2);
    assert_eq!(after_turn.player.temp_strength, 0);
    assert_eq!(after_turn.player.powers.artifact, 0);
}

#[test]
fn headbutt_plus_auto_places_single_discard_card_on_draw_pile() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HEADBUTT_PLUS_ID)];
    state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), cards::DEFEND_R_ID)];
    state.piles.draw_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Headbutt+ plays against an enemy");

    assert!(next.discard_select().is_none());
    assert_eq!(next.monsters[0].hp, starting_hp - 12);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::HEADBUTT_PLUS_ID
    );
}

#[test]
fn lethal_headbutt_defers_gremlin_horn_until_after_discard_choice() {
    let target = MonsterId::new(1);
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    state.relics = vec![Relic::GremlinHorn];
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HEADBUTT_ID)];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), cards::BASH_ID)];
    state.piles.discard_pile = vec![
        CardInstance::new(CardId::new(3), cards::RAMPAGE_ID),
        CardInstance::new(CardId::new(4), cards::DEFEND_R_ID),
    ];
    let mut dying = monster_state(&FIXED_SIMPLE_MONSTER, target);
    dying.hp = 9;
    dying.max_hp = 9;
    let survivor = monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2));
    state.monsters = vec![dying, survivor];

    let mut selecting = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(target),
        },
    )
    .expect("Headbutt kills the first monster");

    assert!(selecting.discard_select().is_some());
    assert_eq!(selecting.player.energy, 2);
    assert_eq!(selecting.pending_monster_death_relic_triggers, 1);
    assert_eq!(selecting.piles.draw_pile[0].content_id, cards::BASH_ID);

    choose_discard_select(&mut selecting, 0).expect("select Rampage");
    confirm_headbutt_select(&mut selecting).expect("confirm Headbutt selection");

    assert!(selecting.discard_select().is_none());
    assert_eq!(selecting.pending_monster_death_relic_triggers, 0);
    assert_eq!(selecting.player.energy, 3);
    assert_eq!(
        selecting.piles.hand.last().map(|card| card.content_id),
        Some(cards::RAMPAGE_ID),
        "Gremlin Horn draws the card Headbutt just placed on top"
    );
    assert_eq!(selecting.piles.draw_pile[0].content_id, cards::BASH_ID);
}

#[test]
fn heavy_blade_plus_uses_five_times_positive_strength() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.player.powers.strength = 3;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::HEAVY_BLADE_PLUS_ID,
    )];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Heavy Blade+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 29);
    assert_eq!(next.player.energy, 0);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::HEAVY_BLADE_PLUS_ID
    );
}

#[test]
fn havoc_played_anger_adds_generated_copy_before_source_discard() {
    let mut state = CombatState::initial_fixture();
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), cards::ANGER_PLUS_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_play_top_draw_card_action(&state, Some(MonsterId::new(1)))
        .expect("top-draw Anger+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 8);
    assert_eq!(next.piles.discard_pile.len(), 2);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::ANGER_PLUS_ID);
    assert!(!next.piles.discard_pile[0].combat_only);
    assert_eq!(next.piles.discard_pile[1].content_id, cards::ANGER_PLUS_ID);
    assert!(!next.piles.discard_pile[1].combat_only);
}

#[test]
fn hemokinesis_plus_deals_damage_before_rupture_strength_from_hp_loss() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.hp = 50;
    state.player.powers.rupture = 2;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::HEMOKINESIS_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::BLOOD_FOR_BLOOD_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Hemokinesis+ plays against an enemy");

    assert_eq!(next.player.hp, 48);
    assert_eq!(next.monsters[0].hp, starting_hp - 20);
    assert_eq!(next.player.powers.strength, 2);
    assert_eq!(next.piles.hand[0].content_id, cards::BLOOD_FOR_BLOOD_ID);
    assert_eq!(next.piles.hand[0].blood_for_blood_cost_reduction, 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::HEMOKINESIS_PLUS_ID
    );
}

#[test]
fn immolate_plus_deals_all_enemies_and_generates_combat_only_burn() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::IMMOLATE_PLUS_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1)),
        monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)),
    ];
    let starting_hp = state
        .monsters
        .iter()
        .map(|monster| monster.hp)
        .collect::<Vec<_>>();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Immolate+ plays without a selected target");

    assert_eq!(next.monsters[0].hp, starting_hp[0] - 28);
    assert_eq!(next.monsters[1].hp, starting_hp[1] - 28);
    assert_eq!(next.piles.discard_pile.len(), 2);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::BURN_ID);
    assert!(next.piles.discard_pile[0].combat_only);
    assert_eq!(
        next.piles.discard_pile[1].content_id,
        cards::IMMOLATE_PLUS_ID
    );
}

#[test]
fn iron_wave_plus_gains_block_before_spikes_reflect_damage() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.hp = 50;
    state.player.block = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::IRON_WAVE_PLUS_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].powers.spikes = 7;
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Iron Wave+ plays against a spiked enemy");

    assert_eq!(next.player.hp, 50);
    assert_eq!(next.player.block, 0);
    assert_eq!(next.monsters[0].hp, starting_hp - 7);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::IRON_WAVE_PLUS_ID
    );
}

#[test]
fn dropkick_plus_draws_and_refunds_energy_against_vulnerable_enemy() {
    assert_eq!(cards::DROPKICK.values.damage, Some(5));
    assert_eq!(cards::DROPKICK_PLUS.values.damage, Some(8));

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DROPKICK_PLUS_ID)];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), cards::WOUND_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].powers.vulnerable = 1;
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Dropkick+ plays against a vulnerable enemy");

    assert_eq!(next.player.energy, 1);
    assert_eq!(next.monsters[0].hp, starting_hp - 12);
    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::WOUND_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::DROPKICK_PLUS_ID
    );
}

#[test]
fn feed_plus_gains_four_max_hp_on_fatal_non_minion_hit_and_exhausts() {
    assert_eq!(cards::FEED.values.damage, Some(10));
    assert!(cards::FEED.keywords.exhaust);
    assert_eq!(cards::FEED_PLUS.values.damage, Some(12));
    assert!(cards::FEED_PLUS.keywords.exhaust);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.hp = 40;
    state.player.max_hp = 50;
    state.relics.clear();
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::FEED_PLUS_ID)];
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 12;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Feed+ plays against an enemy");

    assert!(!next.monsters[0].alive);
    assert_eq!(next.player.max_hp, 54);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::FEED_PLUS_ID);
}

#[test]
fn exhume_plus_is_playable_with_no_exhumable_cards_and_exhausts() {
    assert_eq!(cards::EXHUME.cost, 1);
    assert!(cards::EXHUME.keywords.exhaust);
    assert_eq!(cards::EXHUME_PLUS.cost, 0);
    assert!(cards::EXHUME_PLUS.keywords.exhaust);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::EXHUME_PLUS_ID)];
    state.piles.exhaust_pile.clear();

    assert!(
        valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
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
    .expect("Exhume+ can play with no exhumable cards");

    assert!(next.exhaust_select().is_none());
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::EXHUME_PLUS_ID);
}

#[test]
fn exhume_plus_auto_returns_the_only_exhumable_card() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::EXHUME_PLUS_ID)];
    state.piles.exhaust_pile = vec![CardInstance::new(CardId::new(2), cards::WOUND_ID)];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Exhume+ auto-returns a sole exhumable card");

    assert!(next.exhaust_select().is_none());
    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::WOUND_ID);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::EXHUME_PLUS_ID);
}

#[test]
fn fiend_fire_plus_hits_once_per_other_hand_card_and_exhausts_all() {
    assert_eq!(cards::FIEND_FIRE.values.damage, Some(7));
    assert!(cards::FIEND_FIRE.keywords.exhaust);
    assert_eq!(cards::FIEND_FIRE_PLUS.values.damage, Some(10));
    assert!(cards::FIEND_FIRE_PLUS.keywords.exhaust);

    let mut state = CombatState::initial_fixture();
    state.player.energy = 2;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FIEND_FIRE_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::WOUND_ID),
        CardInstance::new(CardId::new(3), cards::SLIMED_ID),
    ];
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Fiend Fire+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 20);
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 3);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::FIEND_FIRE_PLUS_ID));
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
        valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
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
fn bite_plus_heals_three_even_when_damage_is_blocked() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = 50;
    state.player.max_hp = 60;
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BITE_PLUS_ID)];
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
    .expect("Bite+ plays against a blocked target");

    assert_eq!(next.player.hp, 53);
    assert_eq!(next.monsters[0].hp, starting_monster_hp);
    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::BITE_PLUS_ID);
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
fn whirlwind_is_playable_at_zero_energy_for_zero_hits() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::WHIRLWIND_ID)];
    state.piles.discard_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_monster_hp = state.monsters[0].hp;
    let play = CombatAction::PlayCard {
        card_id: CardId::new(1),
        target: None,
    };

    assert!(valid_legal_combat_actions(&state).contains(&play));
    let next = apply_combat_action(&state, play)
        .expect("the real game permits zero-energy Whirlwind for zero hits");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_monster_hp);
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::WHIRLWIND_ID);
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
fn transmutation_plus_generates_an_upgraded_zero_cost_colorless_card() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::TRANSMUTATION_PLUS_ID,
    )];
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Transmutation+ plays");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.hand.len(), 1);
    let generated = next.piles.hand[0];
    assert_eq!(generated.temp_cost, Some(0));
    assert!(generated.temp_cost_turn_only);
    assert!(colorless_discovery_pool()
        .iter()
        .any(|base| { cards::upgrade_content_id(*base) == Some(generated.content_id) }));
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::TRANSMUTATION_PLUS_ID
    );
}

#[test]
fn transmutation_can_be_played_with_zero_energy() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::TRANSMUTATION_ID)];
    state.piles.exhaust_pile.clear();

    let play = CombatAction::PlayCard {
        card_id: CardId::new(1),
        target: None,
    };
    assert!(valid_legal_combat_actions(&state).contains(&play));

    let next = apply_combat_action(&state, play).expect("zero-energy Transmutation plays");
    assert_eq!(next.player.energy, 0);
    assert!(next.piles.hand.is_empty());
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::TRANSMUTATION_ID
    );
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
    let card_random_counter = state.rng.card_random_rng.counter();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Discovery+ plays without a target");

    assert_eq!(next.player.energy, 0);
    assert_eq!(
        next.rng.card_random_rng.counter(),
        card_random_counter + 3,
        "Discovery opening consumes only its visible three-card offer"
    );
    assert!(next.piles.hand.is_empty());
    assert!(next.piles.exhaust_pile.is_empty());
    assert!(next.discovery_card_reward_choices().is_some());
    assert_eq!(
        match next.decision.as_ref() {
            Some(sts_core::CombatDecisionState::DiscoveryCardReward { source_card, .. }) =>
                source_card.as_ref().map(|card| card.content_id),
            _ => None,
        },
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
        valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
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

    assert!(next.hand_select().is_none());
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

    assert!(next.hand_select().is_none());
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
    // The selected Bash is removed from the visible choice list, so Defend is
    // now visible at UI index 0.
    choose_hand_select(&mut next, 0).expect("select Defend");
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
fn ritual_dagger_damage_overflow_fails_before_resolving_the_hit() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    let mut ritual_dagger = CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID);
    ritual_dagger.ritual_dagger_damage_bonus = i32::MAX;
    state.piles.hand = vec![ritual_dagger];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.monsters[0].hp = 15;
    state.monsters[0].max_hp = 15;

    assert_eq!(
        apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(MonsterId::new(1)),
            },
        ),
        Err(sts_core::SimError::InvalidState(
            "Ritual Dagger damage overflows i32"
        ))
    );
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
fn ritual_dagger_does_not_grow_on_a_half_dead_darkling() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID)];
    state.piles.exhaust_pile.clear();
    // Living sibling keeps Life Link from permanently killing the pack.
    state.monsters = vec![
        monster_state(&DARKLING_A0, MonsterId::new(1)),
        monster_state(&DARKLING_A0, MonsterId::new(2)),
    ];
    for monster in &mut state.monsters {
        monster.rolled_attack_damage = Some(8);
        monster.intent = MonsterIntent::Attack { damage: 8 };
    }
    state.monsters[0].hp = 1;
    state.monsters[0].max_hp = 1;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Ritual Dagger puts the Darkling into its half-dead state");

    assert!(!next.monsters[0].alive);
    assert!(next.monsters[0].escaped);
    assert!(next.monsters[1].alive);
    assert_eq!(next.piles.exhaust_pile[0].ritual_dagger_damage_bonus, 0);
}

#[test]
fn upgraded_ritual_dagger_grows_by_five_without_changing_content_id() {
    let upgraded =
        cards::upgrade_card_instance(CardInstance::new(CardId::new(1), cards::RITUAL_DAGGER_ID))
            .expect("Ritual Dagger upgrade is representable")
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
fn madness_ignores_fully_reduced_blood_for_blood() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    let mut blood_for_blood = CardInstance::new(CardId::new(2), cards::BLOOD_FOR_BLOOD_ID);
    blood_for_blood.blood_for_blood_cost_reduction = 4;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::MADNESS_ID),
        blood_for_blood,
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
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

    let blood_for_blood = next
        .piles
        .hand
        .iter()
        .find(|card| card.id == CardId::new(2))
        .expect("Blood for Blood remains in hand");
    let strike = next
        .piles
        .hand
        .iter()
        .find(|card| card.id == CardId::new(3))
        .expect("Strike remains in hand");
    assert_eq!(blood_for_blood.temp_cost, None);
    assert_eq!(strike.temp_cost, Some(0));
    assert!(!strike.temp_cost_turn_only);
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
        state.rng.card_random_rng = StsRng::new(seed);
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
fn havoc_empty_draw_and_discard_discards_source_without_top_card_effect() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile.clear();
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;
    let rng_before = state.rng.card_random_rng.counter();

    let legal_actions = valid_legal_combat_actions(&state);
    assert!(legal_actions.contains(&CombatAction::PlayCard {
        card_id: CardId::new(1),
        target: None,
    }));

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Havoc no-ops when draw and discard piles are empty");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_hp);
    assert!(next.piles.draw_pile.is_empty());
    assert!(next.piles.exhaust_pile.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::HAVOC_ID);
    // Havoc.use always draws a random living target via cardRandomRng before
    // queuing PlayTopCardAction, even when that action later no-ops.
    assert_eq!(
        next.rng.card_random_rng.counter(),
        rng_before + 1,
        "empty Havoc still consumes the Havoc.use target roll"
    );
}

#[test]
fn havoc_empty_draw_shuffles_discard_then_plays_top_card() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_ID)];
    state.piles.draw_pile.clear();
    state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), cards::ANGER_PLUS_ID)];
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let legal_actions = valid_legal_combat_actions(&state);
    assert!(legal_actions.contains(&CombatAction::PlayCard {
        card_id: CardId::new(1),
        target: None,
    }));

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Havoc shuffles discard into draw and randomly targets the top card");

    assert_eq!(next.player.energy, 0);
    assert_eq!(next.monsters[0].hp, starting_hp - 8);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(next.piles.exhaust_pile[0].content_id, cards::ANGER_PLUS_ID);
    assert!(next
        .piles
        .discard_pile
        .iter()
        .any(|card| card.content_id == cards::HAVOC_ID));
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
fn battle_trance_under_corruption_sets_no_draw_before_dark_embrace() {
    // Battle Trance must apply No Draw before Corruption exhausts it, or
    // Dark Embrace would draw a fourth card after the trance draws.
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.player.powers.corruption = 1;
    state.player.powers.dark_embrace = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BATTLE_TRANCE_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(4), cards::BASH_ID),
        CardInstance::new(CardId::new(5), cards::FLEX_ID),
        CardInstance::new(CardId::new(6), cards::ANGER_ID),
    ];
    state.piles.exhaust_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Battle Trance under Corruption");

    assert!(next.player.cannot_draw);
    assert_eq!(
        next.piles.hand.len(),
        3,
        "exactly three draws; Dark Embrace must not add a fourth: {:?}",
        next.piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>()
    );
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::BATTLE_TRANCE_ID));
    assert_eq!(next.piles.draw_pile.len(), 2);
}

#[test]
fn battle_trance_draw_does_not_trigger_evolve_extra_draws() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    state.player.powers.evolve = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::BATTLE_TRANCE_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::DAZED_ID),
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
    .expect("Battle Trance should resolve its fixed draw count");

    assert_eq!(next.piles.hand.len(), 3);
    assert!(next.piles.draw_pile.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert!(next.player.cannot_draw);
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
    assert!(next.piles.exhaust_pile.is_empty());
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
    assert!(next.piles.exhaust_pile.is_empty());
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

    start_player_turn(&mut state).expect("player turn starts");

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
    assert!(next.exhaust_select().is_some());

    for _ in 0..5 {
        choose_exhaust_select(&mut next, 0).expect("select next hand card");
    }
    confirm_exhaust_select(&mut next).expect("confirm Purity+ exhaust selection");

    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 6);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::PURITY_PLUS_ID));
    assert!(next.exhaust_select().is_none());
}

#[test]
fn hand_played_purity_source_is_hidden_until_confirm() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::PURITY_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile.clear();
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("hand-played Purity opens exhaust selection");

    assert!(next.exhaust_select().is_some());
    assert!(
        next.piles.exhaust_pile.is_empty(),
        "hand-played Purity stays out of the visible exhaust pile until selection is confirmed"
    );
    assert_eq!(next.piles.hand.len(), 2);

    choose_exhaust_select(&mut next, 0).expect("select Defend");
    confirm_exhaust_select(&mut next).expect("confirm Purity selection");

    assert_eq!(next.piles.exhaust_pile.len(), 2);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::PURITY_ID));
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == cards::DEFEND_R_ID));
}

#[test]
fn top_draw_entrench_plus_doubles_current_block_and_discards_source() {
    let mut state = CombatState::initial_fixture();
    state.player.block = 9;
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), cards::ENTRENCH_PLUS_ID)];
    state.piles.discard_pile.clear();

    let next = apply_play_top_draw_card_action(&state, None)
        .expect("top-draw Entrench+ doubles current block");

    assert_eq!(next.player.block, 18);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::ENTRENCH_PLUS_ID
    );
}

#[test]
fn top_draw_flame_barrier_plus_gains_block_and_temporary_thorns() {
    let mut state = CombatState::initial_fixture();
    state.player.block = 0;
    state.player.temp_thorns = 0;
    state.piles.draw_pile = vec![CardInstance::new(
        CardId::new(1),
        cards::FLAME_BARRIER_PLUS_ID,
    )];
    state.piles.discard_pile.clear();

    let next = apply_play_top_draw_card_action(&state, None)
        .expect("top-draw Flame Barrier+ grants block and thorns");

    assert_eq!(next.player.block, 16);
    assert_eq!(next.player.temp_thorns, 6);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::FLAME_BARRIER_PLUS_ID
    );
}

#[test]
fn top_draw_flex_plus_grants_temporary_strength() {
    let mut state = CombatState::initial_fixture();
    state.player.temp_strength = 0;
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), cards::FLEX_PLUS_ID)];
    state.piles.discard_pile.clear();

    let next =
        apply_play_top_draw_card_action(&state, None).expect("top-draw Flex+ grants Strength");

    assert_eq!(next.player.temp_strength, 4);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::FLEX_PLUS_ID);
}

#[test]
fn strike_plus_deals_nine_damage_and_discards() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::STRIKE_R_PLUS_ID)];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike+ plays against an enemy");

    assert_eq!(next.monsters[0].hp, starting_hp - 9);
    assert_eq!(next.player.energy, 0);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::STRIKE_R_PLUS_ID
    );
}

#[test]
fn top_draw_secret_technique_fetches_skill_and_exhausts_source() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand.clear();
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(1), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_PLUS_ID),
        CardInstance::new(CardId::new(4), cards::SECRET_TECHNIQUE_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let mut next = apply_play_top_draw_card_action(&state, None)
        .expect("top-draw Secret Technique opens draw selection");
    assert!(next.draw_select().is_some());

    let defend_index = next
        .draw_select()
        .expect("draw selection")
        .selectable_card_ids
        .iter()
        .position(|id| *id == CardId::new(2))
        .expect("Defend is selectable");
    choose_draw_select(&mut next, defend_index).expect("select Defend skill");
    confirm_draw_select(&mut next).expect("confirm Secret Technique draw selection");

    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert!(next.piles.discard_pile.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::SECRET_TECHNIQUE_ID
    );
    assert!(next.draw_select().is_none());
}

#[test]
fn secret_technique_with_one_skill_moves_it_without_a_grid() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand = vec![CardInstance::new(
        CardId::new(1),
        cards::SECRET_TECHNIQUE_ID,
    )];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
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
    .expect("Secret Technique plays with one skill in draw pile");

    assert!(next.draw_select().is_none());
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::SECRET_TECHNIQUE_ID
    );
}

#[test]
fn top_draw_secret_weapon_plus_fetches_attack_and_discards_source() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand.clear();
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(1), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::BASH_ID),
        CardInstance::new(CardId::new(4), cards::SECRET_WEAPON_PLUS_ID),
    ];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();

    let mut next = apply_play_top_draw_card_action(&state, None)
        .expect("top-draw Secret Weapon+ opens draw selection");
    assert!(next.draw_select().is_some());

    let strike_index = next
        .draw_select()
        .expect("draw selection")
        .selectable_card_ids
        .iter()
        .position(|id| *id == CardId::new(2))
        .expect("Strike is selectable");
    choose_draw_select(&mut next, strike_index).expect("select Strike attack");
    confirm_draw_select(&mut next).expect("confirm Secret Weapon+ draw selection");

    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.hand[0].content_id, cards::STRIKE_R_ID);
    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::DEFEND_R_ID);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        cards::SECRET_WEAPON_PLUS_ID
    );
    assert!(next.piles.exhaust_pile.is_empty());
    assert!(next.draw_select().is_none());
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
fn havoc_played_attack_resolves_damage_before_shuriken_strength() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 1;
    state.player.powers.strength = 2;
    state.relics.push(Relic::Shuriken);
    state.relic_counters.shuriken_attacks_this_turn = 2;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::HAVOC_PLUS_ID)];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), cards::POMMEL_STRIKE_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    let starting_hp = state.monsters[0].hp;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Havoc+ should play top-deck Pommel Strike");

    assert_eq!(next.monsters[0].hp, starting_hp - 11);
    assert_eq!(next.player.powers.strength, 3);
    assert_eq!(next.relic_counters.shuriken_attacks_this_turn, 0);
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    assert_eq!(
        next.piles.exhaust_pile[0].content_id,
        cards::POMMEL_STRIKE_ID
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
fn deep_breath_uses_separate_discard_and_draw_shuffle_rng_calls() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 0;
    state.rng.shuffle_rng = StsRng::new(123);
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::DEEP_BREATH_ID)];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(2), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(3), cards::RAMPAGE_ID),
    ];
    state.piles.discard_pile = vec![
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::BASH_ID),
    ];
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Deep Breath plays with a non-empty discard pile");

    assert_eq!(next.rng.shuffle_rng.counter(), 2);
    assert_eq!(next.piles.hand.len(), 1);
    assert_eq!(next.piles.draw_pile.len(), 3);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, cards::DEEP_BREATH_ID);
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
        valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
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
        valid_legal_combat_actions(&state).contains(&CombatAction::PlayCard {
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
