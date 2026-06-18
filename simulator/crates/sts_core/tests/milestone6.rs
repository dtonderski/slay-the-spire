use sts_core::{
    content::monsters::{CULTIST_A0, JAW_WORM_A0},
    end_player_turn, CombatState, MonsterIntent,
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
