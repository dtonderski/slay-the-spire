use sts_core::{content::monsters::CULTIST_A0, end_player_turn, CombatState, MonsterIntent};

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
