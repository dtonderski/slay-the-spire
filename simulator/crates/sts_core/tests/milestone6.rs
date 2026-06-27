use sts_core::{
    apply_combat_action,
    content::cards::{BURN_ID, DAZED_ID, DEFEND_R_ID},
    content::monsters::{
        guardian_on_hp_damage, ACID_SLIME_A0, CULTIST_A0, GREEN_LOUSE_A0, GREMLIN_NOB_A0,
        GUARDIAN_A0, HEXAGHOST_A0, JAW_WORM_A0, LAGAVULIN_A0, RED_LOUSE_A0, SENTRY_A0,
        SLIME_BOSS_A0, SPIKE_SLIME_A0,
    },
    end_player_turn, CardId, CombatAction, CombatState, MonsterId, MonsterIntent,
};

#[test]
fn cultist_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::cultist_fixture();

    assert_eq!(state.monsters[0].hp, CULTIST_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::Ritual {
            amount: CULTIST_A0.ritual_amount
        }
    );
}

#[test]
fn cultist_combat_executes_ritual_then_scaling_attack() {
    let mut state = CombatState::cultist_fixture();
    state.player.hp = 40;
    state.piles.draw_pile.clear();

    let after_ritual = end_player_turn(&state);

    assert_eq!(after_ritual.player.hp, 40);
    assert_eq!(after_ritual.monsters[0].powers.strength, 2);
    assert_eq!(
        after_ritual.monsters[0].intent,
        MonsterIntent::Attack {
            damage: CULTIST_A0.attack_damage
        }
    );

    let after_attack = end_player_turn(&after_ritual);

    assert_eq!(after_attack.player.hp, 32);
}

#[test]
fn jaw_worm_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::jaw_worm_fixture();

    assert_eq!(state.monsters[0].hp, JAW_WORM_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::Attack { damage: 11 }
    );
}

#[test]
fn jaw_worm_combat_executes_chomp_thrash_bellow_cycle() {
    let mut state = CombatState::jaw_worm_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_chomp = end_player_turn(&state);
    assert_eq!(after_chomp.player.hp, 89);
    assert_eq!(
        after_chomp.monsters[0].intent,
        MonsterIntent::AttackAndBlock {
            damage: 7,
            block: 5,
        }
    );

    let after_thrash = end_player_turn(&after_chomp);
    assert_eq!(after_thrash.player.hp, 82);
    assert_eq!(after_thrash.monsters[0].block, 5);
    assert_eq!(
        after_thrash.monsters[0].intent,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 6,
        }
    );

    let after_bellow = end_player_turn(&after_thrash);
    assert_eq!(after_bellow.player.hp, 82);
    assert_eq!(after_bellow.monsters[0].powers.strength, 3);
    assert_eq!(after_bellow.monsters[0].block, 6);
    assert_eq!(
        after_bellow.monsters[0].intent,
        MonsterIntent::Attack { damage: 11 }
    );

    let after_second_chomp = end_player_turn(&after_bellow);
    assert_eq!(after_second_chomp.player.hp, 68);
}

#[test]
fn gremlin_nob_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::gremlin_nob_fixture();

    assert_eq!(state.monsters[0].hp, GREMLIN_NOB_A0.hp);
    assert_eq!(state.monsters[0].intent, MonsterIntent::Block { block: 0 });
}

#[test]
fn gremlin_nob_combat_executes_bellow_skull_bash_rush_cycle() {
    let mut state = CombatState::gremlin_nob_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_bellow = end_player_turn(&state);
    assert_eq!(after_bellow.player.hp, 100);
    assert_eq!(
        after_bellow.monsters[0].intent,
        MonsterIntent::AttackApplyPlayerVulnerable {
            damage: 6,
            vulnerable: 2,
        }
    );

    let after_skull_bash = end_player_turn(&after_bellow);
    assert_eq!(after_skull_bash.player.hp, 94);
    assert_eq!(after_skull_bash.player.powers.vulnerable, 2);
    assert_eq!(
        after_skull_bash.monsters[0].intent,
        MonsterIntent::Attack { damage: 14 }
    );

    let after_first_rush = end_player_turn(&after_skull_bash);
    assert_eq!(after_first_rush.player.hp, 73);
    assert_eq!(after_first_rush.player.powers.vulnerable, 1);

    let after_second_rush = end_player_turn(&after_first_rush);
    assert_eq!(after_second_rush.player.hp, 52);
    assert_eq!(after_second_rush.player.powers.vulnerable, 0);
}

#[test]
fn gremlin_nob_enrage_applies_anger_when_player_plays_skill() {
    let state = CombatState::gremlin_nob_fixture();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_defend_id(&state),
            target: None,
        },
    )
    .expect("Defend applies");

    assert_eq!(next.monsters[0].powers.anger, 2);
    assert_eq!(next.monsters[0].powers.strength, 2);
}

#[test]
fn gremlin_nob_enrage_bonus_is_applied_once_to_next_attack() {
    let mut state = CombatState::gremlin_nob_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_defend = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_defend_id(&state),
            target: None,
        },
    )
    .expect("Defend applies");
    let after_bellow = end_player_turn(&after_defend);
    let after_skull_bash = end_player_turn(&after_bellow);

    assert_eq!(after_skull_bash.player.hp, 92);
    assert_eq!(after_skull_bash.monsters[0].powers.anger, 2);
    assert_eq!(after_skull_bash.monsters[0].powers.strength, 2);
}

#[test]
fn gremlin_nob_enrage_does_not_trigger_on_attack() {
    let state = CombatState::gremlin_nob_fixture();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_strike_id(&state),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike applies");

    assert_eq!(next.monsters[0].powers.anger, 0);
    assert_eq!(next.monsters[0].powers.strength, 0);
}

#[test]
fn red_louse_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::red_louse_fixture();

    assert_eq!(state.monsters[0].hp, RED_LOUSE_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 0
        }
    );
}

#[test]
fn red_louse_combat_executes_curl_bite_cycle() {
    let mut state = CombatState::red_louse_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_curl = end_player_turn(&state);
    assert_eq!(after_curl.player.hp, 100);
    assert_eq!(after_curl.monsters[0].block, 0);
    assert_eq!(
        after_curl.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );

    let after_bite = end_player_turn(&after_curl);
    assert_eq!(after_bite.player.hp, 91);
    assert_eq!(
        after_bite.monsters[0].intent,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 0
        }
    );

    let after_second_curl = end_player_turn(&after_bite);
    assert_eq!(after_second_curl.player.hp, 91);
    assert_eq!(after_second_curl.monsters[0].block, 0);
    assert_eq!(after_second_curl.monsters[0].powers.strength, 6);
    assert_eq!(
        after_second_curl.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );
}

#[test]
fn green_louse_fixture_has_expected_hp_spikes_and_opening_intent() {
    let state = CombatState::green_louse_fixture();

    assert_eq!(state.monsters[0].hp, GREEN_LOUSE_A0.hp);
    assert_eq!(state.monsters[0].powers.spikes, 3);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 0
        }
    );
}

#[test]
fn green_louse_spikes_reflect_damage_when_struck() {
    let state = CombatState::green_louse_fixture();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_strike_id(&state),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike applies");

    assert_eq!(next.monsters[0].hp, 6);
    assert_eq!(next.player.hp, 77);
}

#[test]
fn green_louse_combat_executes_curl_bite_cycle() {
    let mut state = CombatState::green_louse_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_curl = end_player_turn(&state);
    assert_eq!(after_curl.player.hp, 100);
    assert_eq!(after_curl.monsters[0].block, 0);
    assert_eq!(
        after_curl.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );

    let after_bite = end_player_turn(&after_curl);
    assert_eq!(after_bite.player.hp, 91);
}

#[test]
fn spike_slime_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::spike_slime_fixture();

    assert_eq!(state.monsters[0].hp, SPIKE_SLIME_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::Attack { damage: 5 }
    );
}

#[test]
fn spike_slime_combat_executes_spit_lick_cycle() {
    let mut state = CombatState::spike_slime_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_spit = end_player_turn(&state);
    assert_eq!(after_spit.player.hp, 95);
    assert_eq!(after_spit.player.powers.weak, 0);
    assert_eq!(
        after_spit.monsters[0].intent,
        MonsterIntent::ApplyPlayerWeak { amount: 1 }
    );

    let after_lick = end_player_turn(&after_spit);
    assert_eq!(after_lick.player.hp, 95);
    assert_eq!(after_lick.player.powers.weak, 1);
    assert_eq!(
        after_lick.monsters[0].intent,
        MonsterIntent::Attack { damage: 5 }
    );
}

#[test]
fn acid_slime_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::acid_slime_fixture();

    assert_eq!(state.monsters[0].hp, ACID_SLIME_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::ApplyPlayerWeak { amount: 1 }
    );
}

#[test]
fn acid_slime_combat_executes_weak_attack_cycle() {
    let mut state = CombatState::acid_slime_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_weak = end_player_turn(&state);
    assert_eq!(after_weak.player.hp, 100);
    assert_eq!(after_weak.player.powers.weak, 1);
    assert_eq!(
        after_weak.monsters[0].intent,
        MonsterIntent::Attack { damage: 7 }
    );

    let after_attack = end_player_turn(&after_weak);
    assert_eq!(after_attack.player.hp, 93);
    assert_eq!(
        after_attack.monsters[0].intent,
        MonsterIntent::ApplyPlayerWeak { amount: 1 }
    );
}

#[test]
fn lagavulin_fixture_has_expected_hp_and_sleep_intent() {
    let state = CombatState::lagavulin_fixture();

    assert_eq!(state.monsters[0].hp, LAGAVULIN_A0.hp);
    assert_eq!(state.monsters[0].sleep_turns_remaining, 3);
    assert_eq!(state.monsters[0].intent, MonsterIntent::Sleep);
}

#[test]
fn lagavulin_sleeps_three_turns_then_siphons_and_attacks() {
    let mut state = CombatState::lagavulin_fixture();
    state.player.hp = 100;
    state.player.powers.strength = 3;
    state.player.powers.dexterity = 2;
    state.piles.draw_pile.clear();

    let after_sleep_one = end_player_turn(&state);
    assert_eq!(after_sleep_one.player.hp, 100);
    assert_eq!(after_sleep_one.monsters[0].sleep_turns_remaining, 2);

    let after_sleep_two = end_player_turn(&after_sleep_one);
    assert_eq!(after_sleep_two.player.hp, 100);
    assert_eq!(after_sleep_two.monsters[0].sleep_turns_remaining, 1);

    let after_sleep_three = end_player_turn(&after_sleep_two);
    assert_eq!(after_sleep_three.player.hp, 100);
    assert_eq!(after_sleep_three.monsters[0].sleep_turns_remaining, 0);
    assert_eq!(
        after_sleep_three.monsters[0].intent,
        MonsterIntent::SiphonPlayer {
            strength: 2,
            dexterity: 2,
        }
    );

    let after_siphon = end_player_turn(&after_sleep_three);
    assert_eq!(after_siphon.player.hp, 100);
    assert_eq!(after_siphon.player.powers.strength, 1);
    assert_eq!(after_siphon.player.powers.dexterity, 0);
    assert_eq!(
        after_siphon.monsters[0].intent,
        MonsterIntent::Attack { damage: 18 }
    );

    let after_attack = end_player_turn(&after_siphon);
    assert_eq!(after_attack.player.hp, 82);
}

#[test]
fn lagavulin_wake_on_strike_stuns_then_attacks() {
    let mut state = CombatState::lagavulin_fixture();
    state.player.hp = 100;
    state.player.powers.strength = 3;
    state.player.powers.dexterity = 2;
    state.piles.draw_pile.clear();

    let after_strike = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_strike_id(&state),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike applies");

    assert_eq!(after_strike.monsters[0].sleep_turns_remaining, 0);
    assert_eq!(after_strike.monsters[0].intent, MonsterIntent::Stun);

    let after_turn = end_player_turn(&after_strike);
    assert_eq!(after_turn.player.hp, 100);
    assert_eq!(after_turn.player.powers.strength, 3);
    assert_eq!(after_turn.player.powers.dexterity, 2);
    assert_eq!(
        after_turn.monsters[0].intent,
        MonsterIntent::Attack { damage: 18 }
    );

    let after_attack = end_player_turn(&after_turn);
    assert_eq!(after_attack.player.hp, 82);
    assert_eq!(after_attack.player.powers.strength, 3);
    assert_eq!(after_attack.player.powers.dexterity, 2);
}

#[test]
fn sentry_fixture_has_three_enemies_with_beam_intent() {
    let state = CombatState::sentry_fixture();

    assert_eq!(state.monsters.len(), 3);
    assert_eq!(state.monsters[0].hp, SENTRY_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::AddDazedToDiscard { count: 2 }
    );
}

#[test]
fn sentry_encounter_beams_then_attacks() {
    let mut state = CombatState::sentry_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_beam = end_player_turn(&state);
    assert_eq!(after_beam.player.hp, 100);
    assert_eq!(
        after_beam
            .piles
            .discard_pile
            .iter()
            .filter(|card| card.content_id == DAZED_ID)
            .count(),
        6
    );
    assert_eq!(
        after_beam.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );

    let after_attack = end_player_turn(&after_beam);
    assert_eq!(after_attack.player.hp, 82);
}

#[test]
fn hexaghost_fixture_has_expected_hp_and_divider_intent() {
    let state = CombatState::hexaghost_fixture();

    assert_eq!(state.monsters[0].hp, HEXAGHOST_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::AttackMultiple { damage: 6, hits: 2 }
    );
}

#[test]
fn hexaghost_combat_executes_divider_tackle_inferno_cycle() {
    let mut state = CombatState::hexaghost_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_divider = end_player_turn(&state);
    assert_eq!(after_divider.player.hp, 88);
    assert_eq!(
        after_divider.monsters[0].intent,
        MonsterIntent::AttackMultiple { damage: 5, hits: 6 }
    );

    let after_tackle = end_player_turn(&after_divider);
    assert_eq!(after_tackle.player.hp, 58);
    assert_eq!(
        after_tackle.monsters[0].intent,
        MonsterIntent::AddBurnToDiscard {
            count: 3,
            damage: 2,
        }
    );

    let after_inferno = end_player_turn(&after_tackle);
    assert_eq!(after_inferno.player.hp, 56);
    assert_eq!(
        after_inferno
            .piles
            .discard_pile
            .iter()
            .filter(|card| card.content_id == BURN_ID)
            .count(),
        3
    );
    assert_eq!(
        after_inferno.monsters[0].intent,
        MonsterIntent::AttackMultiple { damage: 6, hits: 2 }
    );
}

#[test]
fn slime_boss_fixture_has_expected_hp_and_slam_intent() {
    let state = CombatState::slime_boss_fixture();

    assert_eq!(state.monsters[0].hp, SLIME_BOSS_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::Attack { damage: 35 }
    );
}

#[test]
fn slime_boss_splits_into_acid_slimes_at_half_hp() {
    let mut state = CombatState::slime_boss_fixture();
    state.monsters[0].hp = 71;

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_strike_id(&state),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike applies");

    assert_eq!(next.monsters[0].hp, 65);
    assert!(next.monsters[0].split_triggered);
    assert_eq!(next.monsters.len(), 3);
    assert_eq!(next.monsters[1].content_id, ACID_SLIME_A0.content_id);
    assert_eq!(next.monsters[2].content_id, ACID_SLIME_A0.content_id);
}

#[test]
fn guardian_fixture_has_expected_hp_and_charge_up_intent() {
    let state = CombatState::guardian_fixture();

    assert_eq!(state.monsters[0].hp, GUARDIAN_A0.hp);
    assert_eq!(state.monsters[0].mode_shift, 30);
    assert_eq!(state.monsters[0].intent, MonsterIntent::Block { block: 9 });
}

#[test]
fn guardian_mode_shift_enters_defensive_on_hp_damage() {
    let mut state = CombatState::guardian_fixture();
    state.player.hp = 200;
    state.piles.draw_pile.clear();
    state.monsters[0].mode_shift = 10;

    let mut after = state.clone();
    after.monsters[0].hp -= 10;
    guardian_on_hp_damage(&mut after.monsters[0], 10);

    assert!(after.monsters[0].in_defensive_mode);
    assert_eq!(after.monsters[0].powers.spikes, 0);
    assert_eq!(
        after.monsters[0].intent,
        MonsterIntent::GuardianCloseUp { sharp_hide: 3 }
    );
}

#[test]
fn guardian_close_up_turn_applies_sharp_hide_without_attacking() {
    let mut state = CombatState::guardian_fixture();
    state.player.hp = 200;
    state.piles.draw_pile.clear();
    state.monsters[0].mode_shift = 1;

    let after_shift = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_strike_id(&state),
            target: Some(MonsterId::new(1)),
        },
    )
    .expect("Strike applies");
    let after_close_up = end_player_turn(&after_shift);

    assert_eq!(after_close_up.player.hp, 200);
    assert_eq!(after_close_up.monsters[0].block, 0);
    assert_eq!(after_close_up.monsters[0].powers.spikes, 3);
    assert_eq!(
        after_close_up.monsters[0].intent,
        MonsterIntent::Attack { damage: 9 }
    );
}

fn hand_strike_id(state: &CombatState) -> CardId {
    state
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == sts_core::content::cards::STRIKE_R_ID)
        .expect("Strike is in hand")
        .id
}

fn hand_defend_id(state: &CombatState) -> CardId {
    state
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == DEFEND_R_ID)
        .expect("Defend is in hand")
        .id
}
