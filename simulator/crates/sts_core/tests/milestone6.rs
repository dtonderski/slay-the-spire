use sts_core::{
    apply_combat_action,
    content::cards::DEFEND_R_ID,
    content::monsters::{CULTIST_A0, GREMLIN_NOB_A0, JAW_WORM_A0},
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
