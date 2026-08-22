use sts_core::{
    apply_combat_action, apply_combat_action_with_events,
    combat::CombatOrb,
    content::{
        cards::{
            BANE_ANY_COLOR_ID, CAPACITOR_ANY_COLOR_ID, CONCLUDE_ANY_COLOR_ID, HALT_ANY_COLOR_ID,
            RECURSION_ANY_COLOR_ID,
        },
        monsters::{monster_state, BRONZE_ORB_A0},
    },
    CardId, CardInstance, CombatAction, CombatState, InternalAction, MonsterId, MonsterIntent,
};

fn only_card(state: &mut CombatState, content_id: sts_core::ContentId) -> CardId {
    let id = CardId::new(100);
    state.piles.hand = vec![CardInstance::new(id, content_id)];
    state.piles.draw_pile.clear();
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    id
}

#[test]
fn conclude_hits_all_enemies_before_forcing_end_turn() {
    let mut state = CombatState::initial_fixture();
    let mut second = state.monsters[0].clone();
    second.id = MonsterId::new(2);
    state.monsters.push(second);
    for monster in &mut state.monsters {
        monster.hp = 40;
        monster.max_hp = 40;
        monster.intent = MonsterIntent::Stun;
    }
    let card_id = only_card(&mut state, CONCLUDE_ANY_COLOR_ID);
    state.piles.draw_pile = (101..=105)
        .map(|id| CardInstance::new(CardId::new(id), sts_core::content::cards::STRIKE_R_ID))
        .collect();

    let transition = apply_combat_action_with_events(
        &state,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Conclude resolves");

    assert!(transition
        .state
        .monsters
        .iter()
        .all(|monster| monster.hp == 28));
    let damage_index = transition
        .event_log
        .iter()
        .position(|action| matches!(action, InternalAction::DealDamageAll { amount: 12, .. }))
        .expect("all-enemy damage action");
    let end_index = transition
        .event_log
        .iter()
        .position(|action| matches!(action, InternalAction::ForceEndTurn))
        .expect("forced end-turn request");
    let source_move_index = transition
        .event_log
        .iter()
        .position(|action| {
            matches!(
                action,
                InternalAction::MoveCard { card_id: moved, .. } if *moved == card_id
            )
        })
        .expect("Conclude source movement");
    let settlement_index = transition
        .event_log
        .iter()
        .position(|action| matches!(action, InternalAction::SettleForcedEndTurn))
        .expect("forced end-turn settlement");
    assert!(damage_index < end_index);
    assert!(end_index < source_move_index);
    assert!(source_move_index < settlement_index);
    assert_eq!(
        transition
            .state
            .piles
            .discard_pile
            .iter()
            .filter(|card| card.id == card_id)
            .count(),
        1,
        "end-turn discard must not move Conclude twice"
    );
}

#[test]
fn bane_resolves_a_separate_second_hit_only_for_live_poisoned_target() {
    for (poison, upgrades, expected_hp) in [(0, 0, 93), (1, 0, 86), (1, 1, 80)] {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 100;
        state.monsters[0].max_hp = 100;
        state.monsters[0].powers.poison = poison;
        let target = state.monsters[0].id;
        let card_id = only_card(&mut state, BANE_ANY_COLOR_ID);
        state.piles.hand[0].upgrades = upgrades;

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id,
                target: Some(target),
            },
        )
        .expect("Bane resolves");

        assert_eq!(transition.state.monsters[0].hp, expected_hp);
        let first = transition
            .event_log
            .iter()
            .position(|action| matches!(action, InternalAction::DealDamage { .. }))
            .expect("first hit");
        let second = transition
            .event_log
            .iter()
            .position(|action| matches!(action, InternalAction::DealBaneDamageIfPoisoned { .. }))
            .expect("separate conditional BaneAction");
        assert!(first < second);
    }

    let mut lethal = CombatState::initial_fixture();
    lethal.monsters[0].hp = 7;
    lethal.monsters[0].max_hp = 7;
    lethal.monsters[0].powers.poison = 1;
    let target = lethal.monsters[0].id;
    let card_id = only_card(&mut lethal, BANE_ANY_COLOR_ID);
    let transition = apply_combat_action_with_events(
        &lethal,
        CombatAction::PlayCard {
            card_id,
            target: Some(target),
        },
    )
    .expect("lethal first Bane hit skips the conditional second hit");
    assert_eq!(transition.state.monsters[0].hp, 0);
    assert!(!transition.state.monsters[0].alive);
}

#[test]
fn halt_uses_one_gain_block_action_for_source_total() {
    for (wrath, upgrades, expected) in [(false, 0, 3), (false, 1, 4), (true, 0, 12), (true, 1, 18)]
    {
        let mut state = CombatState::initial_fixture();
        state.player.block = 0;
        state.player.powers.wrath = i32::from(wrath);
        let card_id = only_card(&mut state, HALT_ANY_COLOR_ID);
        state.piles.hand[0].upgrades = upgrades;

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id,
                target: None,
            },
        )
        .expect("Halt resolves");

        assert_eq!(transition.state.player.block, expected);
        let gains = transition
            .event_log
            .iter()
            .filter_map(|action| match action {
                InternalAction::GainPrecomputedCardBlock { amount } => Some(*amount),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(gains, vec![expected]);
    }
}

#[test]
fn halt_applies_dexterity_and_frail_to_each_source_component() {
    let mut dexterity = CombatState::initial_fixture();
    dexterity.player.powers.wrath = 1;
    dexterity.player.powers.dexterity = 1;
    let card_id = only_card(&mut dexterity, HALT_ANY_COLOR_ID);
    let dexterity = apply_combat_action(
        &dexterity,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Halt resolves with Dexterity");
    assert_eq!(dexterity.player.block, 14, "(3 + 1) + (9 + 1)");

    let mut frail = CombatState::initial_fixture();
    frail.player.powers.wrath = 1;
    frail.player.powers.frail = 1;
    let card_id = only_card(&mut frail, HALT_ANY_COLOR_ID);
    let frail = apply_combat_action(
        &frail,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Halt resolves while Frail");
    assert_eq!(frail.player.block, 8, "floor(3 * .75) + floor(9 * .75)");
}

#[test]
fn recursion_plus_spends_confusion_cost_for_turn() {
    let mut state = CombatState::initial_fixture();
    state.player.energy = 3;
    state.max_orbs = 1;
    state.orbs = vec![CombatOrb::Frost];
    let card_id = only_card(&mut state, RECURSION_ANY_COLOR_ID);
    state.piles.hand[0].upgrades = 1;
    state.piles.hand[0].temp_cost = Some(2);
    state.piles.hand[0].temp_cost_turn_only = true;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Recursion+ resolves");

    assert_eq!(next.player.energy, 1);
    assert_eq!(next.player.block, 5);
    assert_eq!(next.orbs, vec![CombatOrb::Frost]);
}

#[test]
fn dark_evoke_targets_first_lowest_hp_without_rng_or_attack_modifiers() {
    let mut state = CombatState::initial_fixture();
    let mut second = state.monsters[0].clone();
    second.id = MonsterId::new(2);
    let mut third = state.monsters[0].clone();
    third.id = MonsterId::new(3);
    state.monsters.extend([second, third]);
    for (monster, hp) in state.monsters.iter_mut().zip([30, 20, 20]) {
        monster.hp = hp;
        monster.max_hp = hp;
    }
    state.player.powers.strength = 100;
    state.pen_nib_double_active = true;
    state.max_orbs = 1;
    state.orbs = vec![CombatOrb::Dark { evoke: 7 }];
    let card_id = only_card(&mut state, RECURSION_ANY_COLOR_ID);
    state.piles.hand[0].temp_cost = Some(0);
    let rng_before = state.rng.card_random_rng.clone();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Recursion evokes Dark");

    assert_eq!(
        next.monsters
            .iter()
            .map(|monster| monster.hp)
            .collect::<Vec<_>>(),
        vec![30, 13, 20]
    );
    assert_eq!(next.rng.card_random_rng, rng_before);
}

#[test]
fn dark_evoke_applies_lock_on_before_unmodified_thorns_damage() {
    let mut state = CombatState::initial_fixture();
    state.monsters[0].hp = 20;
    state.monsters[0].max_hp = 20;
    state.monsters[0].powers.lock_on = 1;
    state.player.powers.strength = 100;
    state.max_orbs = 1;
    state.orbs = vec![CombatOrb::Dark { evoke: 7 }];
    let card_id = only_card(&mut state, RECURSION_ANY_COLOR_ID);
    state.piles.hand[0].temp_cost = Some(0);

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Dark evoke resolves through Lock-On");

    assert_eq!(next.monsters[0].hp, 10, "floor(7 * 1.5) damage");
}

#[test]
fn capacitor_plus_can_overshoot_ten_orb_slots() {
    let mut state = CombatState::initial_fixture();
    state.max_orbs = 9;
    let card_id = only_card(&mut state, CAPACITOR_ANY_COLOR_ID);
    state.piles.hand[0].upgrades = 1;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Capacitor+ resolves from nine slots");

    assert_eq!(next.max_orbs, 12);
    next.validate().expect("overshot orb slots remain valid");
}

#[test]
fn dark_evoke_uses_complete_unmodified_death_aftermath() {
    let mut state = CombatState::initial_fixture();
    let mut bronze_orb = monster_state(&BRONZE_ORB_A0, MonsterId::new(2));
    bronze_orb.hp = 6;
    bronze_orb.max_hp = 6;
    bronze_orb.stasis_card = Some(CardInstance::new(
        CardId::new(200),
        sts_core::content::cards::BASH_ID,
    ));
    state.monsters[0].hp = 20;
    state.monsters[0].max_hp = 20;
    state.monsters.push(bronze_orb);
    state.max_orbs = 1;
    state.orbs = vec![CombatOrb::Dark { evoke: 6 }];
    let card_id = only_card(&mut state, RECURSION_ANY_COLOR_ID);
    state.piles.hand[0].temp_cost = Some(0);

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id,
            target: None,
        },
    )
    .expect("Dark evoke resolves death hooks");

    assert!(!next.monsters[1].alive);
    assert!(next.monsters[1].stasis_card.is_none());
    assert!(next
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == sts_core::content::cards::BASH_ID));
}
