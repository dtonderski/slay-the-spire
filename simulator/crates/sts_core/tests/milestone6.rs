use sts_core::{
    apply_combat_action,
    content::cards::DEFEND_R_ID,
    content::monsters::{
        ACID_SLIME_A0, CULTIST_A0, GREEN_LOUSE_A0, GREMLIN_NOB_A0, JAW_WORM_A0, LAGAVULIN_A0,
        RED_LOUSE_A0, SPIKE_SLIME_A0,
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
    assert_eq!(after_bellow.monsters[0].block, 11);
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
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );
}

#[test]
fn gremlin_nob_combat_executes_bite_skull_bash_rush_cycle() {
    let mut state = CombatState::gremlin_nob_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_bite = end_player_turn(&state);
    assert_eq!(after_bite.player.hp, 94);
    assert_eq!(
        after_bite.monsters[0].intent,
        MonsterIntent::Attack { damage: 14 }
    );

    let after_skull_bash = end_player_turn(&after_bite);
    assert_eq!(after_skull_bash.player.hp, 80);
    assert_eq!(
        after_skull_bash.monsters[0].intent,
        MonsterIntent::Attack { damage: 10 }
    );

    let after_rush = end_player_turn(&after_skull_bash);
    assert_eq!(after_rush.player.hp, 70);
    assert_eq!(
        after_rush.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );
}

#[test]
fn gremlin_nob_enrage_applies_weak_when_player_plays_skill() {
    let state = CombatState::gremlin_nob_fixture();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: hand_defend_id(&state),
            target: None,
        },
    )
    .expect("Defend applies");

    assert_eq!(next.player.powers.weak, 2);
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

    assert_eq!(next.player.powers.weak, 0);
}

#[test]
fn red_louse_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::red_louse_fixture();

    assert_eq!(state.monsters[0].hp, RED_LOUSE_A0.hp);
    assert_eq!(state.monsters[0].intent, MonsterIntent::Block { block: 3 });
}

#[test]
fn red_louse_combat_executes_curl_bite_cycle() {
    let mut state = CombatState::red_louse_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_curl = end_player_turn(&state);
    assert_eq!(after_curl.player.hp, 100);
    assert_eq!(after_curl.monsters[0].block, 3);
    assert_eq!(
        after_curl.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );

    let after_bite = end_player_turn(&after_curl);
    assert_eq!(after_bite.player.hp, 94);
    assert_eq!(
        after_bite.monsters[0].intent,
        MonsterIntent::Block { block: 3 }
    );

    let after_second_curl = end_player_turn(&after_bite);
    assert_eq!(after_second_curl.player.hp, 94);
    assert_eq!(after_second_curl.monsters[0].block, 6);
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
    assert_eq!(state.monsters[0].intent, MonsterIntent::Block { block: 3 });
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
    assert_eq!(after_curl.monsters[0].block, 3);
    assert_eq!(
        after_curl.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );

    let after_bite = end_player_turn(&after_curl);
    assert_eq!(after_bite.player.hp, 94);
}

#[test]
fn spike_slime_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::spike_slime_fixture();

    assert_eq!(state.monsters[0].hp, SPIKE_SLIME_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::ApplyPlayerWeak { amount: 1 }
    );
}

#[test]
fn spike_slime_combat_executes_lick_spit_cycle() {
    let mut state = CombatState::spike_slime_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_lick = end_player_turn(&state);
    assert_eq!(after_lick.player.hp, 100);
    assert_eq!(after_lick.player.powers.weak, 1);
    assert_eq!(
        after_lick.monsters[0].intent,
        MonsterIntent::Attack { damage: 7 }
    );

    let after_spit = end_player_turn(&after_lick);
    assert_eq!(after_spit.player.hp, 93);
    assert_eq!(
        after_spit.monsters[0].intent,
        MonsterIntent::ApplyPlayerWeak { amount: 1 }
    );
}

#[test]
fn acid_slime_fixture_has_expected_hp_and_opening_intent() {
    let state = CombatState::acid_slime_fixture();

    assert_eq!(state.monsters[0].hp, ACID_SLIME_A0.hp);
    assert_eq!(
        state.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
    );
}

#[test]
fn acid_slime_combat_executes_attack_weak_cycle() {
    let mut state = CombatState::acid_slime_fixture();
    state.player.hp = 100;
    state.piles.draw_pile.clear();

    let after_attack = end_player_turn(&state);
    assert_eq!(after_attack.player.hp, 94);
    assert_eq!(
        after_attack.monsters[0].intent,
        MonsterIntent::ApplyPlayerWeak { amount: 1 }
    );

    let after_weak = end_player_turn(&after_attack);
    assert_eq!(after_weak.player.hp, 94);
    assert_eq!(after_weak.player.powers.weak, 1);
    assert_eq!(
        after_weak.monsters[0].intent,
        MonsterIntent::Attack { damage: 6 }
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
fn lagavulin_wake_on_strike_siphons_on_same_monster_turn() {
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
    assert_eq!(
        after_strike.monsters[0].intent,
        MonsterIntent::SiphonPlayer {
            strength: 2,
            dexterity: 2,
        }
    );

    let after_turn = end_player_turn(&after_strike);
    assert_eq!(after_turn.player.hp, 100);
    assert_eq!(after_turn.player.powers.strength, 1);
    assert_eq!(after_turn.player.powers.dexterity, 0);
    assert_eq!(
        after_turn.monsters[0].intent,
        MonsterIntent::Attack { damage: 18 }
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
