//! Source-backed synthetic boundaries, not dedicated real-game trace parity.
//! RedSkull.onBloodied/onNotBloodied apply +3/-3 StrengthPower, whose stacking
//! bounds the current total to +/-999, not its split bookkeeping component.
use sts_core::adapter_internals::{
    apply_combat_action, CardId, CardInstance, CombatAction, CombatState, Relic, SimError,
};
use sts_core::content::cards::{BANDAGE_UP_ID, BLOODLETTING_ID, STRIKE_R_ID};

fn setup(base: i32, loss: i32, active: bool) -> CombatState {
    let mut c = CombatState::initial_fixture();
    c.player.authority.relics = vec![Relic::RedSkull];
    c.player.hp = if active { 40 } else { 41 };
    c.player.powers.strength = base;
    c.player.temp_strength = loss;
    c.relic_counters.red_skull_active = active;
    c.piles.hand = vec![
        CardInstance::new(CardId::new(1), BLOODLETTING_ID),
        CardInstance::new(CardId::new(2), BANDAGE_UP_ID),
        CardInstance::new(CardId::new(3), STRIKE_R_ID),
    ];
    c.monsters[0].hp = 5000;
    c.monsters[0].max_hp = 5000;
    c.validate().unwrap();
    c
}

fn play(c: &CombatState, id: u64, attack: bool) -> CombatState {
    let action = CombatAction::PlayCard {
        card_id: CardId::new(id),
        target: attack.then_some(c.monsters[0].id),
    };
    let before = serde_json::to_value(c).unwrap();
    let restored: CombatState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_combat_action(c, action).unwrap();
    let repeat = apply_combat_action(&restored, action).unwrap();
    assert_eq!(serde_json::to_value(c).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeat).unwrap()
    );
    assert_eq!(c.rng, next.rng);
    next.validate().unwrap();
    println!(
        "red_skull_bounds_transition={}",
        serde_json::json!({"initial": before,"action": action,"result": next})
    );
    next
}

fn current(c: &CombatState) -> i32 {
    c.player.powers.strength + c.player.temp_strength
}

#[test]
fn activation_caps_visible_strength() {
    for strength in [997, 998, 999] {
        let next = play(&setup(strength, 0, false), 1, false);
        assert!(next.relic_counters.red_skull_active);
        assert_eq!(current(&next), 999);
    }
}

#[test]
fn unprotected_removal_caps_negative_strength() {
    for strength in [-997, -998, -999] {
        let next = play(&setup(strength, 0, true), 2, false);
        assert!(!next.relic_counters.red_skull_active);
        assert_eq!(current(&next), -999);
    }
}

#[test]
fn removal_uses_nominal_three_after_clipped_activation() {
    let activated = play(&setup(998, 0, false), 1, false);
    let healed = play(&activated, 2, false);
    assert!(!healed.relic_counters.red_skull_active);
    assert_eq!(current(&healed), 996);
}

#[test]
fn activation_caps_combined_strength_and_keeps_full_pending_loss() {
    for (base, loss) in [(975, 24), (-1, 1000)] {
        let next = play(&setup(base, loss, false), 1, false);
        assert_eq!(current(&next), 999);
        assert_eq!(next.player.temp_strength, loss);
        assert_eq!(next.player.powers.strength, base);
    }
}

#[test]
fn removal_caps_combined_strength_without_erasing_loss() {
    let next = play(&setup(-1000, 2, true), 2, false);
    assert_eq!(current(&next), -999);
    assert_eq!(next.player.temp_strength, 2);
    assert_eq!(next.player.powers.strength, -1001);
}

#[test]
fn ordinary_activation_and_removal_are_unchanged() {
    let activated = play(&setup(0, 0, false), 1, false);
    assert_eq!(current(&activated), 3);
    assert_eq!(current(&play(&activated, 2, false)), 0);
}

#[test]
fn overflow_rejects_atomically_with_existing_activation_error() {
    let c = setup(i32::MAX, 0, false);
    let before = serde_json::to_value(&c).unwrap();
    assert_eq!(
        apply_combat_action(
            &c,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None
            }
        ),
        Err(SimError::InvalidState(
            "Red Skull Strength activation overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(c).unwrap(), before);
}

#[test]
fn clipped_strength_controls_subsequent_strike_damage() {
    let activated = play(&setup(998, 0, false), 1, false);
    let attacked = play(&activated, 3, true);
    assert_eq!(activated.monsters[0].hp - attacked.monsters[0].hp, 1005);
}
